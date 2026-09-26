//! Scalar conditional, cleanup and call plan fixtures and the x86 and
//! AArch64 unit call accounts.

use super::dynamic_and_cleanup_plans::add_empty_unit_cleanup;
use super::provider_and_call_plans::{internal_call_plan, two_function_plan};
use super::{edge_id, machine_id, operation_id};
use calling_conventions::{ValuePlacement, ValueShape};
use machine_code::{
    Aarch64ReturnLinkEvidence, FunctionFragmentConditionalBranchPredicate, InternalCallRelocation,
    InternalUnitCallRecord, MachineCodeFunction, MachineCodePlan, ScalarCallStackEvidence,
    ScalarCleanupPreservationEvidence, ScalarConditionalBranchEvidence, ScalarConditionalCondition,
    ScalarControlAffineCleanupRecord, ScalarControlBlockEvidence, ScalarControlFlowEvidence,
    ScalarControlTerminatorEvidence, ScalarDirectConditionalBranchEvidence, ScalarStackEvidence,
    ScalarStackMutation, ScalarStackMutationKind, SemanticCodeAttribution, SemanticCodeSite,
    StackAdjustmentPair, UnitAffineCleanupRecord, UnitCallStackEvidence, UnitParameterHomeRecord,
    UnitParameterRecord, UnitStackEvidence,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use target::{Architecture, NativeTarget};
use terminal_psi::{StructuralMultiplicity, TerminalAffineCleanupAction};

pub(super) fn scalar_two_return_conditional_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.target = target;
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    match target.architecture {
        target::Architecture::X86_64 => {
            function.bytes = vec![
                0x89, 0xf8, // mov eax, edi
                0x85, 0xc0, // test eax, eax
                0x0f, 0x84, 9, 0, 0, 0, // jz false arm
                0x48, 0x89, 0xf0, // true arm
                0x25, 0xff, 0, 0, 0, 0xc3, // ret
                0x48, 0x89, 0xd0, // false arm
                0x25, 0xff, 0, 0, 0, 0xc3, // ret
            ];
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: Vec::new(),
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 4, 6, 19),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
        target::Architecture::Aarch64 => {
            function.bytes = aarch64_words(&[
                0x3400_0080, // cbz w0, false arm at byte 16
                0xd100_43ff, // true: sub sp, sp, #16
                0x9100_43ff, // true: add sp, sp, #16
                0xd65f_03c0, // true: ret
                0xd100_83ff, // false: sub sp, sp, #32
                0x9100_83ff, // false: add sp, sp, #32
                0xd65f_03c0, // false: ret
            ]);
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(8, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(16, 4, ScalarStackMutationKind::Allocate { byte_size: 32 }),
                    scalar_mutation(20, 4, ScalarStackMutationKind::Release { byte_size: 32 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 0, 4, 16),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
    }
    plan
}

/// One no-call scalar function whose complete forward graph is retained as
/// `Acyclic` evidence: a conditional selects between a plain arm and an arm
/// carrying a balanced stack mutation, and both reconverge on one shared
/// return. Every transfer terminator is owned by a nonzero edge attribution
/// row and the conditional also owns a zero-width fallthrough row, so object
/// construction can reconstruct the block topology from the final bytes alone
/// before replaying each region's claimed stack mutations.
pub(super) fn scalar_acyclic_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.target = target;
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    // One edge identity per retained attribution row.
    function.provenance.edges = (1..=5).map(edge_id).collect();
    let attribution = |edge, operation_ordinal, code_offset, byte_count| SemanticCodeAttribution {
        site: SemanticCodeSite::Edge(edge_id(edge)),
        operation_ordinal,
        code_offset,
        byte_count,
    };
    match target.architecture {
        Architecture::X86_64 => {
            function.bytes = vec![
                0x48, 0x39, 0xf7, // cmp rdi, rsi
                0x75, 10, // jnz +10: taken arm at 15
                0xb8, 0, 0, 0, 0, // fallthrough arm: mov eax, 0
                0xe9, 12, 0, 0, 0,    // jmp +12: shared return at 27
                0x50, // taken arm: push rax
                0xb8, 7, 0, 0, 0,    // mov eax, 7
                0x58, // pop rax
                0xe9, 0, 0, 0, 0,    // jmp +0: shared return at 27
                0xc3, // shared return
            ];
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(15, 1, ScalarStackMutationKind::X86Push),
                    scalar_mutation(21, 1, ScalarStackMutationKind::X86Pop),
                ],
                control_flow: ScalarControlFlowEvidence::Acyclic {
                    blocks: vec![
                        ScalarControlBlockEvidence {
                            offset: 0,
                            byte_count: 5,
                            terminator: ScalarControlTerminatorEvidence::Conditional(
                                ScalarDirectConditionalBranchEvidence {
                                    predicate:
                                        FunctionFragmentConditionalBranchPredicate::NonZeroV1,
                                    branch_offset: 3,
                                    branch_byte_count: 2,
                                    taken_offset: 15,
                                    fallthrough_offset: 5,
                                },
                            ),
                        },
                        ScalarControlBlockEvidence {
                            offset: 5,
                            byte_count: 10,
                            terminator: ScalarControlTerminatorEvidence::Jump {
                                offset: 10,
                                byte_count: 5,
                                target_offset: 27,
                            },
                        },
                        ScalarControlBlockEvidence {
                            offset: 15,
                            byte_count: 12,
                            terminator: ScalarControlTerminatorEvidence::Jump {
                                offset: 22,
                                byte_count: 5,
                                target_offset: 27,
                            },
                        },
                        ScalarControlBlockEvidence {
                            offset: 27,
                            byte_count: 1,
                            terminator: ScalarControlTerminatorEvidence::Return {
                                offset: 27,
                                byte_count: 1,
                            },
                        },
                    ],
                },
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            // Stored in canonical (operation_ordinal, code_offset) order.
            function.semantic_code_attribution = vec![
                attribution(1, 1, 3, 2),  // jnz
                attribution(2, 1, 5, 0),  // jnz fallthrough
                attribution(3, 2, 27, 1), // shared return
                attribution(4, 4, 10, 5), // fallthrough-arm join
                attribution(5, 6, 22, 5), // taken-arm join
            ];
        }
        Architecture::Aarch64 => {
            function.bytes = aarch64_words(&[
                0xeb01_001f, // cmp x0, x1
                0x5400_0061, // b.ne +12: taken arm at 16
                0xd280_0000, // fallthrough arm: mov w0, #0
                0x1400_0005, // b +20: shared return at 32
                0xd100_43ff, // taken arm: sub sp, sp, #16
                0xd280_00e0, // mov w0, #7
                0x9100_43ff, // add sp, sp, #16
                0x1400_0001, // b +4: shared return at 32
                0xd65f_03c0, // shared return
            ]);
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(16, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(24, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: ScalarControlFlowEvidence::Acyclic {
                    blocks: vec![
                        ScalarControlBlockEvidence {
                            offset: 0,
                            byte_count: 8,
                            terminator: ScalarControlTerminatorEvidence::Conditional(
                                ScalarDirectConditionalBranchEvidence {
                                    predicate:
                                        FunctionFragmentConditionalBranchPredicate::NonZeroV1,
                                    branch_offset: 4,
                                    branch_byte_count: 4,
                                    taken_offset: 16,
                                    fallthrough_offset: 8,
                                },
                            ),
                        },
                        ScalarControlBlockEvidence {
                            offset: 8,
                            byte_count: 8,
                            terminator: ScalarControlTerminatorEvidence::Jump {
                                offset: 12,
                                byte_count: 4,
                                target_offset: 32,
                            },
                        },
                        ScalarControlBlockEvidence {
                            offset: 16,
                            byte_count: 16,
                            terminator: ScalarControlTerminatorEvidence::Jump {
                                offset: 28,
                                byte_count: 4,
                                target_offset: 32,
                            },
                        },
                        ScalarControlBlockEvidence {
                            offset: 32,
                            byte_count: 4,
                            terminator: ScalarControlTerminatorEvidence::Return {
                                offset: 32,
                                byte_count: 4,
                            },
                        },
                    ],
                },
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            function.semantic_code_attribution = vec![
                attribution(1, 1, 4, 4),  // b.ne
                attribution(2, 1, 8, 0),  // b.ne fallthrough
                attribution(3, 2, 32, 4), // shared return
                attribution(4, 4, 12, 4), // fallthrough-arm join
                attribution(5, 6, 28, 4), // taken-arm join
            ];
        }
    }
    plan
}

pub(super) fn scalar_three_leaf_cleanup_plan() -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    function.provenance.edges = vec![
        edge_id(1),
        edge_id(2),
        edge_id(3),
        edge_id(4),
        edge_id(10),
        edge_id(11),
        edge_id(12),
    ];
    function.bytes = vec![
        0x85, 0xc0, // test eax, eax
        0x0f, 0x84, 0x38, 0, 0, 0, // root jz third leaf at 64
        0x85, 0xc0, // nested test eax, eax
        0x0f, 0x84, 0x18, 0, 0, 0, // nested jz second leaf at 40
        0xb8, 1, 0, 0, 0, // first result
        0x48, 0x83, 0xec, 16, // first preservation allocation
        0x48, 0x89, 0x44, 0x24, 0, // first result store
        0x48, 0x8b, 0x44, 0x24, 0, // first result load
        0x48, 0x83, 0xc4, 16, 0xc3, // first release/return
        0xb8, 0, 0, 0, 0, // second result
        0x48, 0x83, 0xec, 16, 0x48, 0x89, 0x44, 0x24, 0, 0x48, 0x8b, 0x44, 0x24, 0, 0x48, 0x83,
        0xc4, 16, 0xc3, 0xb8, 1, 0, 0, 0, // third result
        0x48, 0x83, 0xec, 16, 0x48, 0x89, 0x44, 0x24, 0, 0x48, 0x8b, 0x44, 0x24, 0, 0x48, 0x83,
        0xc4, 16, 0xc3,
    ];
    let leaf = |edge: u64, cleanup_start: usize, end: usize| ScalarControlAffineCleanupRecord {
        cleanup: UnitAffineCleanupRecord {
            psi_edge: edge_id(edge),
            structural_types: Vec::new().into(),
            locals: Vec::new(),
            actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(1).unwrap(),
            )],
            code_offset: cleanup_start,
            byte_count: end - cleanup_start,
        },
        preservation: ScalarCleanupPreservationEvidence {
            frame: StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: cleanup_start,
                allocation_byte_count: 4,
                release_offset: end - 5,
                release_byte_count: 4,
            },
            result_byte_offset: 0,
            result_store_offset: cleanup_start + 4,
            result_load_offset: end - 10,
            aarch64_return_link: None,
        },
    };
    function.scalar_control_affine_cleanups =
        vec![leaf(10, 21, 40), leaf(11, 45, 64), leaf(12, 69, 88)];
    function.scalar_stack = Some(ScalarStackEvidence {
        mutations: [21, 45, 69]
            .into_iter()
            .flat_map(|start| {
                [
                    scalar_mutation(
                        start,
                        4,
                        ScalarStackMutationKind::Allocate { byte_size: 16 },
                    ),
                    scalar_mutation(
                        start + 14,
                        4,
                        ScalarStackMutationKind::Release { byte_size: 16 },
                    ),
                ]
            })
            .collect(),
        control_flow: ScalarControlFlowEvidence::ConditionalTree {
            decisions: vec![
                ScalarConditionalBranchEvidence {
                    condition: ScalarConditionalCondition::Parameter,
                    branch_offset: 2,
                    branch_byte_count: 6,
                    false_arm_offset: 64,
                },
                ScalarConditionalBranchEvidence {
                    condition: ScalarConditionalCondition::Parameter,
                    branch_offset: 10,
                    branch_byte_count: 6,
                    false_arm_offset: 40,
                },
            ],
            crash_leaves: vec![false; 3],
            branches: Vec::new(),
        },
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    function.scalar_structural_parameters = vec![UnitParameterRecord {
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        shape: ValueShape::integer(0, 1),
    }];
    function.scalar_structural_parameter_homes = vec![UnitParameterHomeRecord {
        place: PlaceId::new(1).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        shape: ValueShape::integer(0, 1),
        source: ValuePlacement {
            shape: ValueShape::integer(0, 1),
            locations: Vec::new(),
        },
        location: machine_code::StructuralSourceLocation::Stack { byte_offset: 0 },
        indirect: false,
    }];
    function.semantic_code_attribution = [(10, 21, 19), (11, 45, 19), (12, 69, 19)]
        .into_iter()
        .enumerate()
        .map(
            |(ordinal, (edge, code_offset, byte_count))| SemanticCodeAttribution {
                site: SemanticCodeSite::Edge(edge_id(edge)),
                operation_ordinal: ordinal,
                code_offset,
                byte_count,
            },
        )
        .collect();
    plan
}

pub(super) fn scalar_expression_two_return_conditional_plan(
    target: NativeTarget,
) -> MachineCodePlan {
    let mut plan = two_function_plan();
    plan.target = target;
    plan.entry = machine_id(1);
    plan.functions.truncate(1);
    let function = &mut plan.functions[0];
    match target.architecture {
        target::Architecture::X86_64 => {
            function.bytes = vec![
                0x48, 0x83, 0xec, 16, // sub rsp, 16
                0x85, 0xc0, // test eax, eax
                0x48, 0x8d, 0x64, 0x24, 16, // lea rsp, [rsp + 16]
                0x0f, 0x84, 1, 0, 0, 0,    // jz false arm
                0xc3, // true: ret
                0xc3, // false: ret
            ];
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(
                        6,
                        5,
                        ScalarStackMutationKind::X86ReleasePreservingFlags { byte_size: 16 },
                    ),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 11, 6, 18),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
        target::Architecture::Aarch64 => {
            function.bytes = aarch64_words(&[
                0xd100_43ff, // sub sp, sp, #16
                0x7100_001f, // cmp w0, #0
                0x9100_43ff, // add sp, sp, #16
                0x5400_0040, // b.eq false arm at byte 20
                0xd65f_03c0, // true: ret
                0xd65f_03c0, // false: ret
            ]);
            function.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(8, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 12, 4, 20),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
        }
    }
    plan
}

pub(super) fn scalar_expression_condition_call_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = internal_call_plan(target);
    plan.functions[0].bytes = match target.architecture {
        target::Architecture::X86_64 => vec![0xc3],
        target::Architecture::Aarch64 => aarch64_words(&[0xd65f_03c0]),
    };
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let caller = &mut plan.functions[1];
    caller.provenance.operations = vec![operation_id(2)];
    match target.architecture {
        target::Architecture::X86_64 => {
            caller.bytes = vec![
                0x48, 0x83, 0xec, 8, // outbound call area
                0xe8, 0, 0, 0, 0, // typed condition call
                0x48, 0x83, 0xc4, 8, // release call area
                0x85, 0xc0, // test returned Boolean
                0x0f, 0x84, 1, 0, 0, 0,    // jz false arm
                0xc3, // true: ret
                0xc3, // false: ret
            ];
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 8 }),
                    scalar_mutation(9, 4, ScalarStackMutationKind::Release { byte_size: 8 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 15, 6, 22),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![InternalCallRelocation {
                owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                target: machine_id(1),
                unit_stack: None,
                scalar_stack: Some(ScalarCallStackEvidence {
                    outbound: Some(StackAdjustmentPair {
                        byte_size: 8,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 9,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: None,
                }),
                offset: 5,
            }];
        }
        target::Architecture::Aarch64 => {
            caller.bytes = aarch64_words(&[
                0xd100_43ff, // outbound call area
                0xf900_03fe, // save x30
                0x9400_0000, // typed condition call
                0xf940_03fe, // restore x30
                0x9100_43ff, // release call area
                0x7100_001f, // cmp w0, #0
                0x5400_0040, // b.eq false arm at byte 32
                0xd65f_03c0, // true: ret
                0xd65f_03c0, // false: ret
            ]);
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(16, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Expression, 24, 4, 32),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![InternalCallRelocation {
                owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                target: machine_id(1),
                unit_stack: None,
                scalar_stack: Some(ScalarCallStackEvidence {
                    outbound: Some(StackAdjustmentPair {
                        byte_size: 16,
                        allocation_offset: 0,
                        allocation_byte_count: 4,
                        release_offset: 16,
                        release_byte_count: 4,
                    }),
                    aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                        frame_byte_offset: 0,
                        store_offset: 4,
                        load_offset: 12,
                    }),
                }),
                offset: 8,
            }];
        }
    }
    plan
}

pub(super) fn scalar_conditional_call_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = internal_call_plan(target);
    plan.functions[0].bytes = match target.architecture {
        target::Architecture::X86_64 => vec![0xc3],
        target::Architecture::Aarch64 => aarch64_words(&[0xd65f_03c0]),
    };
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let caller = &mut plan.functions[1];
    caller.provenance.operations = vec![operation_id(2), operation_id(3)];
    match target.architecture {
        target::Architecture::X86_64 => {
            caller.bytes = vec![
                0x89, 0xf8, // mov eax, edi
                0x85, 0xc0, // test eax, eax
                0x0f, 0x84, 8, 0, 0, 0,    // jz false arm at byte 18
                0x50, // true: pending temporary
                0xe8, 0, 0, 0, 0,    // true: call
                0x58, // true: restore temporary
                0xc3, // true: ret
                0x48, 0x83, 0xec, 8, // false: outbound allocation
                0xe8, 0, 0, 0, 0, // false: call
                0x48, 0x83, 0xc4, 8,    // false: release
                0xc3, // false: ret
            ];
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(10, 1, ScalarStackMutationKind::X86Push),
                    scalar_mutation(16, 1, ScalarStackMutationKind::X86Pop),
                    scalar_mutation(18, 4, ScalarStackMutationKind::Allocate { byte_size: 8 }),
                    scalar_mutation(27, 4, ScalarStackMutationKind::Release { byte_size: 8 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 4, 6, 18),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: None,
                        aarch64_return_link: None,
                    }),
                    offset: 12,
                },
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(3)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 8,
                            allocation_offset: 18,
                            allocation_byte_count: 4,
                            release_offset: 27,
                            release_byte_count: 4,
                        }),
                        aarch64_return_link: None,
                    }),
                    offset: 23,
                },
            ];
        }
        target::Architecture::Aarch64 => {
            caller.bytes = aarch64_words(&[
                0x3400_0120, // cbz w0, false arm at byte 36
                0xd100_43ff, // true: pending frame
                0xd100_43ff, // true: call area
                0xf900_03fe, // true: save x30
                0x9400_0000, // true: call
                0xf940_03fe, // true: restore x30
                0x9100_43ff, // true: release call area
                0x9100_43ff, // true: release pending frame
                0xd65f_03c0, // true: ret
                0xd100_43ff, // false: call area
                0xf900_03fe, // false: save x30
                0x9400_0000, // false: call
                0xf940_03fe, // false: restore x30
                0x9100_43ff, // false: release call area
                0xd65f_03c0, // false: ret
            ]);
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(8, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(24, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(28, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(36, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(52, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: conditional_tree(ScalarConditionalCondition::Parameter, 0, 4, 36),
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls = vec![
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(2)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 16,
                            allocation_offset: 8,
                            allocation_byte_count: 4,
                            release_offset: 24,
                            release_byte_count: 4,
                        }),
                        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                            frame_byte_offset: 0,
                            store_offset: 12,
                            load_offset: 20,
                        }),
                    }),
                    offset: 16,
                },
                InternalCallRelocation {
                    owner: target_operations::CallSiteOwner::Operation(operation_id(3)),
                    target: machine_id(1),
                    unit_stack: None,
                    scalar_stack: Some(ScalarCallStackEvidence {
                        outbound: Some(StackAdjustmentPair {
                            byte_size: 16,
                            allocation_offset: 36,
                            allocation_byte_count: 4,
                            release_offset: 52,
                            release_byte_count: 4,
                        }),
                        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                            frame_byte_offset: 0,
                            store_offset: 40,
                            load_offset: 48,
                        }),
                    }),
                    offset: 44,
                },
            ];
        }
    }
    plan
}

pub(super) fn scalar_call_plan(target: NativeTarget) -> MachineCodePlan {
    let mut plan = internal_call_plan(target);
    plan.functions[0].bytes = match target.architecture {
        target::Architecture::X86_64 => vec![0xc3],
        target::Architecture::Aarch64 => aarch64_words(&[0xd65f_03c0]),
    };
    plan.functions[0].scalar_stack = Some(ScalarStackEvidence {
        mutations: Vec::new(),
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: None,
    });
    let caller = &mut plan.functions[1];
    match target.architecture {
        target::Architecture::X86_64 => {
            caller.bytes = vec![
                0x50, // pending expression temporary
                0xe8, 0, 0, 0, 0,    // call rel32
                0x58, // restore expression temporary
                0xc3, // ret
            ];
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 1, ScalarStackMutationKind::X86Push),
                    scalar_mutation(6, 1, ScalarStackMutationKind::X86Pop),
                ],
                control_flow: ScalarControlFlowEvidence::Linear,
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls[0].offset = 2;
            caller.internal_calls[0].scalar_stack = Some(ScalarCallStackEvidence {
                outbound: None,
                aarch64_return_link: None,
            });
        }
        target::Architecture::Aarch64 => {
            caller.bytes = aarch64_words(&[
                0xd100_43ff, // pending expression frame: sub sp, sp, #16
                0xd100_43ff, // call area: sub sp, sp, #16
                0xf900_03fe, // str x30, [sp]
                0x9400_0000, // bl #0
                0xf940_03fe, // ldr x30, [sp]
                0x9100_43ff, // release call area
                0x9100_43ff, // release expression frame
                0xd65f_03c0, // ret
            ]);
            caller.scalar_stack = Some(ScalarStackEvidence {
                mutations: vec![
                    scalar_mutation(0, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(4, 4, ScalarStackMutationKind::Allocate { byte_size: 16 }),
                    scalar_mutation(20, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                    scalar_mutation(24, 4, ScalarStackMutationKind::Release { byte_size: 16 }),
                ],
                control_flow: ScalarControlFlowEvidence::Linear,
                stack_alignment: 16,
                cleanup_preservation: None,
            });
            caller.internal_calls[0].offset = 12;
            caller.internal_calls[0].scalar_stack = Some(ScalarCallStackEvidence {
                outbound: Some(StackAdjustmentPair {
                    byte_size: 16,
                    allocation_offset: 4,
                    allocation_byte_count: 4,
                    release_offset: 20,
                    release_byte_count: 4,
                }),
                aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
                    frame_byte_offset: 0,
                    store_offset: 8,
                    load_offset: 16,
                }),
            });
        }
    }
    plan
}

pub(super) fn account_x86_unit_call(plan: &mut MachineCodePlan) {
    let caller = &mut plan.functions[1];
    caller.bytes = vec![
        0x48, 0x83, 0xec, 0x08, // sub rsp, 8
        0xe8, 0, 0, 0, 0, // call rel32
        0x48, 0x83, 0xc4, 0x08, // add rsp, 8
        0xc3, // ret
    ];
    caller.unit_stack = Some(UnitStackEvidence {
        frame: None,
        aarch64_return_link: None,
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(caller);
    caller.internal_calls[0].offset = 5;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence {
        outbound: Some(StackAdjustmentPair {
            byte_size: 8,
            allocation_offset: 0,
            allocation_byte_count: 4,
            release_offset: 9,
            release_byte_count: 4,
        }),
    });
    caller.internal_unit_calls = vec![InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: caller.internal_calls[0].owner,
        target: caller.internal_calls[0].target,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 0,
        byte_count: 13,
    }];
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(
                caller.internal_calls[0]
                    .owner
                    .operation()
                    .expect("ordinary call owner"),
            ),
            operation_ordinal: 0,
            code_offset: 0,
            byte_count: 13,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(caller.provenance.edges[0]),
            operation_ordinal: 1,
            code_offset: 13,
            byte_count: 1,
        },
    ];
}

pub(super) fn scalar_mutation(
    offset: usize,
    byte_count: usize,
    kind: ScalarStackMutationKind,
) -> ScalarStackMutation {
    ScalarStackMutation {
        offset,
        byte_count,
        kind,
    }
}

pub(super) fn conditional_tree(
    condition: ScalarConditionalCondition,
    branch_offset: usize,
    branch_byte_count: usize,
    false_arm_offset: usize,
) -> ScalarControlFlowEvidence {
    ScalarControlFlowEvidence::ConditionalTree {
        decisions: vec![ScalarConditionalBranchEvidence {
            condition,
            branch_offset,
            branch_byte_count,
            false_arm_offset,
        }],
        crash_leaves: vec![false; 2],
        branches: Vec::new(),
    }
}

pub(super) fn promote_x86_cleanup_to_scalar(caller: &mut MachineCodeFunction) {
    let prefix_len = 5;
    caller.bytes.splice(0..0, [0xb8, 1, 0, 0, 0]);
    let cleanup_start = prefix_len;
    caller.bytes.splice(
        cleanup_start..cleanup_start,
        [
            0x48, 0x83, 0xec, 16, // sub rsp, 16
            0x48, 0x89, 0x44, 0x24, 0, // mov [rsp], rax
        ],
    );
    let inserted_prefix = prefix_len + 9;
    let relocation = &mut caller.internal_calls[0];
    relocation.offset += inserted_prefix;
    let outbound = relocation
        .unit_stack
        .take()
        .and_then(|stack| stack.outbound)
        .expect("x86 cleanup call stack pair");
    let outbound = StackAdjustmentPair {
        allocation_offset: outbound.allocation_offset + inserted_prefix,
        release_offset: outbound.release_offset + inserted_prefix,
        ..outbound
    };
    relocation.scalar_stack = Some(ScalarCallStackEvidence {
        outbound: Some(outbound),
        aarch64_return_link: None,
    });
    caller.internal_unit_calls[0].code_offset += inserted_prefix;
    let original_ret = caller.bytes.pop();
    assert_eq!(original_ret, Some(0xc3));
    let result_load_offset = caller.bytes.len();
    caller.bytes.extend_from_slice(&[
        0x48, 0x8b, 0x44, 0x24, 0, // mov rax, [rsp]
        0x48, 0x83, 0xc4, 16, // add rsp, 16
        0xc3,
    ]);
    let frame_release_offset = result_load_offset + 5;
    caller.unit_stack = None;
    caller.scalar_stack = Some(ScalarStackEvidence {
        mutations: vec![
            scalar_mutation(
                cleanup_start,
                4,
                ScalarStackMutationKind::Allocate { byte_size: 16 },
            ),
            scalar_mutation(
                outbound.allocation_offset,
                outbound.allocation_byte_count,
                ScalarStackMutationKind::Allocate {
                    byte_size: outbound.byte_size,
                },
            ),
            scalar_mutation(
                outbound.release_offset,
                outbound.release_byte_count,
                ScalarStackMutationKind::Release {
                    byte_size: outbound.byte_size,
                },
            ),
            scalar_mutation(
                frame_release_offset,
                4,
                ScalarStackMutationKind::Release { byte_size: 16 },
            ),
        ],
        control_flow: ScalarControlFlowEvidence::Linear,
        stack_alignment: 16,
        cleanup_preservation: Some(ScalarCleanupPreservationEvidence {
            frame: StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: cleanup_start,
                allocation_byte_count: 4,
                release_offset: frame_release_offset,
                release_byte_count: 4,
            },
            result_byte_offset: 0,
            result_store_offset: cleanup_start + 4,
            result_load_offset,
            aarch64_return_link: None,
        }),
    });
    let cleanup = caller
        .unit_affine_cleanup
        .take()
        .expect("Unit cleanup fixture");
    caller.scalar_affine_cleanup = Some(UnitAffineCleanupRecord {
        code_offset: cleanup_start,
        byte_count: caller.bytes.len() - cleanup_start,
        ..cleanup
    });
    caller.scalar_structural_parameters = std::mem::take(&mut caller.unit_parameters);
    caller.scalar_structural_parameter_homes = std::mem::take(&mut caller.unit_parameter_homes);
    caller.semantic_code_attribution[0].code_offset += inserted_prefix;
    caller.semantic_code_attribution[0].byte_count += 9;
    let cleanup_fuel = caller
        .semantic_code_attribution
        .last_mut()
        .expect("cleanup edge fuel");
    cleanup_fuel.code_offset = cleanup_start;
    cleanup_fuel.byte_count = caller.bytes.len() - cleanup_start;
}

pub(super) fn account_aarch64_unit_call(plan: &mut MachineCodePlan) {
    let frame = StackAdjustmentPair {
        byte_size: 16,
        allocation_offset: 0,
        allocation_byte_count: 4,
        release_offset: 12,
        release_byte_count: 4,
    };
    let link = Aarch64ReturnLinkEvidence {
        frame_byte_offset: 0,
        store_offset: 4,
        load_offset: 8,
    };
    plan.functions[0].bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xf900_03fe, // str x30, [sp]
        0xf940_03fe, // ldr x30, [sp]
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    plan.functions[0].unit_stack = Some(UnitStackEvidence {
        frame: Some(frame),
        aarch64_return_link: Some(link),
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(&mut plan.functions[0]);

    let caller = &mut plan.functions[1];
    caller.bytes = aarch64_words(&[
        0xd100_43ff, // sub sp, sp, #16
        0xf900_03fe, // str x30, [sp]
        0x9400_0000, // bl immediate
        0xf940_03fe, // ldr x30, [sp]
        0x9100_43ff, // add sp, sp, #16
        0xd65f_03c0, // ret
    ]);
    caller.unit_stack = Some(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            release_offset: 16,
            ..frame
        }),
        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
            load_offset: 12,
            ..link
        }),
        stack_alignment: 16,
    });
    add_empty_unit_cleanup(caller);
    caller.internal_calls[0].offset = 8;
    caller.internal_calls[0].unit_stack = Some(UnitCallStackEvidence { outbound: None });
    caller.internal_unit_calls = vec![InternalUnitCallRecord {
        source: machine_code::InternalUnitCallSource::Authored,
        owner: caller.internal_calls[0].owner,
        target: caller.internal_calls[0].target,
        result: None,
        semantic_result: None,
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments: Vec::new(),
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 8,
        byte_count: 4,
    }];
    caller.semantic_code_attribution = vec![
        SemanticCodeAttribution {
            site: SemanticCodeSite::Operation(
                caller.internal_calls[0]
                    .owner
                    .operation()
                    .expect("ordinary call owner"),
            ),
            operation_ordinal: 0,
            code_offset: 8,
            byte_count: 4,
        },
        SemanticCodeAttribution {
            site: SemanticCodeSite::Edge(caller.provenance.edges[0]),
            operation_ordinal: 1,
            code_offset: 12,
            byte_count: 12,
        },
    ];
}

pub(super) fn aarch64_words(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

pub(super) fn insert_aarch64_word(bytes: &mut Vec<u8>, offset: usize, word: u32) {
    bytes.splice(offset..offset, word.to_le_bytes());
}

pub(super) fn integer_return(value: u8) -> Vec<u8> {
    vec![0xb8, value, 0, 0, 0, 0xc3]
}
