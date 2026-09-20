//! Focused fixed-view-copy computation fixtures.
use super::apply::apply_copy;
use super::source_exit::build_source_exit_copies;
use super::{
    FixedViewCopy, FixedViewCopyError, FixedViewCopyPolicy, IntegerSign,
    RegisterInstructionConstraint, RegisterOperandAccess, ScalarType, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionProvenance, SelectedOperand,
    SelectedTerminator, VirtualFixedConstraintSite, VirtualRegister, VirtualRegisterId,
    VirtualRegisterOrigin,
};

use crate::{
    EntryFixedViewTransition, FixedPrecoloredHomeDomainId, FixedPrecoloredSourceSegmentId,
    FunctionAllocationLegality, LiveRangeEdgeConnector, LiveRangePoint, LivenessPosition,
    VirtualRegisterAllocationLegality,
};
use optimization_unit::ValueDefinitionSite;
use register_model::{
    RegisterClassId, RegisterConstraintFamily, RegisterConstraintId, RegisterConstraintKey,
    RegisterOperandConstraint, RegisterViewId,
};
use selected_instructions::{SelectedBlock, SelectedBlockId, SelectedFunction, SelectedSuccessor};
use semantic_vocabulary::{BlockId, EdgeId, IntegerType, IntegerValue, MachineId, ValueId};

fn key(variant: u32) -> RegisterConstraintKey {
    RegisterConstraintKey {
        family: RegisterConstraintFamily::Instruction,
        variant,
    }
}

fn instruction(
    id: u32,
    kind: SelectedInstructionKind,
    operands: Vec<SelectedOperand>,
) -> SelectedInstruction {
    SelectedInstruction {
        id: SelectedInstructionId(id),
        kind,
        constraint: key(id),
        operands,
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
        provenance: SelectedInstructionProvenance::default(),
    }
}

fn use_operand(
    register: u32,
    class: RegisterClassId,
    view: Option<RegisterViewId>,
) -> SelectedOperand {
    SelectedOperand {
        operand: 0,
        virtual_register: VirtualRegisterId(register),
        access: RegisterOperandAccess::Use,
        class,
        fixed_view: view,
        tied_to: None,
        early_clobber: false,
    }
}

pub(crate) fn fixture() -> (
    SelectedFunction,
    FunctionAllocationLegality,
    RegisterInstructionConstraint,
) {
    let machine = MachineId::new(1).unwrap();
    let class = RegisterClassId(0);
    let from = RegisterViewId(1);
    let to = RegisterViewId(2);
    let source_value = ValueId::new(2).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let compare = instruction(
        0,
        SelectedInstructionKind::CompareI64Zero,
        vec![use_operand(0, class, None)],
    );
    let branch = instruction(
        1,
        SelectedInstructionKind::ConditionalBranchNonZero,
        Vec::new(),
    );
    let return_a = instruction(
        2,
        SelectedInstructionKind::ReturnScalar,
        vec![use_operand(1, class, Some(to))],
    );
    let return_b = instruction(
        3,
        SelectedInstructionKind::ReturnScalar,
        vec![use_operand(1, class, Some(to))],
    );
    let function = SelectedFunction {
        machine,
        attachment: None,
        provenance: Default::default(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![
            VirtualRegister {
                id: VirtualRegisterId(0),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value: ValueId::new(1).unwrap(),
                    parameter_index: 0,
                },
                definition_site: Some(ValueDefinitionSite::FunctionParameter(0)),
                entry_fixed_view: None,
            },
            VirtualRegister {
                id: VirtualRegisterId(1),
                scalar_type: scalar,
                class,
                origin: VirtualRegisterOrigin::EntryParameter {
                    source_value,
                    parameter_index: 1,
                },
                definition_site: Some(ValueDefinitionSite::FunctionParameter(1)),
                entry_fixed_view: Some(from),
            },
        ],
        blocks: vec![
            SelectedBlock {
                id: SelectedBlockId(0),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(1).unwrap(),
                ),
                instructions: vec![compare],
                terminator: SelectedTerminator::ConditionalBranch {
                    instruction: branch,
                    when_nonzero: SelectedSuccessor {
                        role: selected_instructions::SelectedSuccessorRole::Semantic,
                        structural_case: None,
                        structural_bindings: Vec::new(),
                        psi_edge: EdgeId::new(1).unwrap(),
                        block: SelectedBlockId(1),
                        source_target: BlockId::new(2).unwrap(),
                        bindings: Vec::new(),
                        fuel: Vec::new(),
                    },
                    when_zero: SelectedSuccessor {
                        role: selected_instructions::SelectedSuccessorRole::Semantic,
                        structural_case: None,
                        structural_bindings: Vec::new(),
                        psi_edge: EdgeId::new(2).unwrap(),
                        block: SelectedBlockId(2),
                        source_target: BlockId::new(3).unwrap(),
                        bindings: Vec::new(),
                        fuel: Vec::new(),
                    },
                },
            },
            SelectedBlock {
                id: SelectedBlockId(1),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(2).unwrap(),
                ),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: return_a,
                    psi_return_edge: EdgeId::new(3).unwrap(),
                },
            },
            SelectedBlock {
                id: SelectedBlockId(2),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(3).unwrap(),
                ),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: return_b,
                    psi_return_edge: EdgeId::new(4).unwrap(),
                },
            },
        ],
    };
    let site = |instruction| VirtualFixedConstraintSite::Operand {
        position: LivenessPosition(instruction),
        point: LiveRangePoint(instruction),
        instruction: SelectedInstructionId(instruction),
        operand: 0,
        access: RegisterOperandAccess::Use,
    };
    let legality = FunctionAllocationLegality {
        machine,
        virtual_registers: vec![
            VirtualRegisterAllocationLegality {
                virtual_register: VirtualRegisterId(0),
                class,
                points: Vec::new(),
                early_clobber_points: Vec::new(),
                entry_transitions: Vec::new(),
            },
            VirtualRegisterAllocationLegality {
                virtual_register: VirtualRegisterId(1),
                class,
                points: Vec::new(),
                early_clobber_points: Vec::new(),
                entry_transitions: vec![
                    EntryFixedViewTransition {
                        from_view: from,
                        to_site: site(2),
                        to_view: to,
                    },
                    EntryFixedViewTransition {
                        from_view: from,
                        to_site: site(3),
                        to_view: to,
                    },
                ],
            },
        ],
    };
    let row = RegisterInstructionConstraint {
        id: RegisterConstraintId(9),
        key: key(9),
        operands: vec![
            RegisterOperandConstraint {
                operand: 0,
                access: RegisterOperandAccess::Use,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            RegisterOperandConstraint {
                operand: 1,
                access: RegisterOperandAccess::Def,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    (function, legality, row)
}

/// Run the shared-exit builder over a fixture under `policy`, discarding the
/// transformed function.
pub(crate) fn source_exit_copies(
    function: &SelectedFunction,
    references: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    row: &RegisterInstructionConstraint,
    policy: FixedViewCopyPolicy,
    next_instruction: u32,
    next_register: u32,
) -> Result<Vec<FixedViewCopy>, FixedViewCopyError> {
    let mut transformed = function.clone();
    build_source_exit_copies(
        0,
        function,
        references,
        &mut transformed,
        row,
        row.key,
        policy,
        next_instruction,
        next_register,
    )
}

/// Same driver, returning the transformed function alongside the copies.
pub(crate) fn source_exit_copies_transformed(
    function: &SelectedFunction,
    references: &[&super::super::evidence::AuthenticatedFixedViewBoundary],
    row: &RegisterInstructionConstraint,
    policy: FixedViewCopyPolicy,
    next_instruction: u32,
    next_register: u32,
) -> Result<(Vec<FixedViewCopy>, SelectedFunction), FixedViewCopyError> {
    let mut transformed = function.clone();
    build_source_exit_copies(
        0,
        function,
        references,
        &mut transformed,
        row,
        row.key,
        policy,
        next_instruction,
        next_register,
    )
    .map(|copies| (copies, transformed))
}

pub(crate) fn computed_shared_fixture() -> (
    SelectedFunction,
    FunctionAllocationLegality,
    RegisterInstructionConstraint,
    FixedViewCopy,
    SelectedFunction,
) {
    let (function, legality, row) = fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let (mut copies, transformed) = source_exit_copies_transformed(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        4,
        2,
    )
    .unwrap();
    (function, legality, row, copies.remove(0), transformed)
}

pub(crate) fn boundaries(
    legality: &FunctionAllocationLegality,
) -> Vec<super::super::evidence::AuthenticatedFixedViewBoundary> {
    legality.virtual_registers[1]
        .entry_transitions
        .iter()
        .enumerate()
        .map(|(index, transition)| {
            let block = SelectedBlockId(u32::try_from(index + 1).unwrap());
            super::super::evidence::AuthenticatedFixedViewBoundary {
                function: 0,
                machine: legality.machine,
                virtual_register: VirtualRegisterId(1),
                class: legality.virtual_registers[1].class,
                source_segment: FixedPrecoloredSourceSegmentId(0),
                source_domain: FixedPrecoloredHomeDomainId(0),
                from_view: transition.from_view,
                destination_segment: FixedPrecoloredSourceSegmentId(
                    u32::try_from(index + 1).unwrap(),
                ),
                destination_domain: FixedPrecoloredHomeDomainId(u32::try_from(index + 1).unwrap()),
                site: transition.to_site,
                block,
                to_view: transition.to_view,
                incoming: Some(LiveRangeEdgeConnector {
                    source: SelectedBlockId(0),
                    terminator: SelectedInstructionId(1),
                    polarity_ordinal: u8::try_from(index).unwrap(),
                    psi_edge: EdgeId::new(u64::try_from(index + 1).unwrap()).unwrap(),
                    target: block,
                }),
            }
        })
        .collect()
}

#[test]
fn shared_entry_policy_inserts_one_copy_at_the_source_exit_and_rewrites_both_returns() {
    let (_, _, _, copy, transformed) = computed_shared_fixture();
    // The member connectors exit the entry block's branch, so the single
    // copy lands at the end of that block and dominates both leaf sites.
    assert_eq!(copy.insertion_block, SelectedBlockId(0));
    assert_eq!(copy.before_instruction, SelectedInstructionId(1));
    assert_eq!(copy.destinations.len(), 2);
    assert_eq!(transformed.blocks[0].instructions.len(), 2);
    assert_eq!(
        transformed.blocks[0].instructions[1].kind,
        SelectedInstructionKind::CopyI64
    );
    for leaf in &transformed.blocks[1..] {
        let SelectedTerminator::Return { instruction, .. } = &leaf.terminator else {
            panic!()
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            VirtualRegisterId(2)
        );
        assert!(leaf.instructions.is_empty());
    }
}

/// The shared-exit leg places the copy by connector evidence, not by the
/// entry block's shape: a longer entry instruction list no longer refuses,
/// and the copy still lands before the shared branch terminator.
#[test]
fn shared_entry_policy_no_longer_templates_the_entry_block_shape() {
    let (mut function, legality, row) = fixture();
    function.blocks[0].instructions.push(instruction(
        4,
        SelectedInstructionKind::CopyI64,
        Vec::new(),
    ));
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, transformed) = source_exit_copies_transformed(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
        5,
        2,
    )
    .unwrap();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].insertion_block, SelectedBlockId(0));
    assert_eq!(copies[0].before_instruction, SelectedInstructionId(1));
    assert_eq!(transformed.blocks[0].instructions.len(), 3);
    assert_eq!(
        transformed.blocks[0].instructions[2].kind,
        SelectedInstructionKind::CopyI64
    );
}

/// Two derivations over the identical boundary set produce the identical
/// copy and transformed function; the declared selection refuses any
/// boundary the partition cannot share — an empty set emits nothing while a
/// lone boundary or a member dropped to a site copy refuses — and the
/// transformed function is terminal: its rewritten site operands no longer
/// read the source register, so re-admission cannot reconstruct the copy.
#[test]
fn shared_entry_copy_is_deterministic_bounded_and_terminal() {
    let (function, legality, row) = fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let declared = FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1;
    let first = source_exit_copies(&function, &references, &row, declared, 4, 2).unwrap();
    let second = source_exit_copies(&function, &references, &row, declared, 4, 2).unwrap();
    assert_eq!(first, second);
    let mut transformed_a = function.clone();
    apply_copy(0, &mut transformed_a, &first[0], &row).unwrap();
    let mut transformed_b = function.clone();
    apply_copy(0, &mut transformed_b, &second[0], &row).unwrap();
    assert_eq!(transformed_a, transformed_b);
    // Fixed point: the member sites were rewritten onto the copy result, so
    // the same boundary set can no longer find the source register at them.
    assert_eq!(
        source_exit_copies(&transformed_a, &references, &row, declared, 5, 3),
        Err(FixedViewCopyError::MissingDestination {
            function: 0,
            instruction: 2
        })
    );
    // No boundaries emit no copies; a lone boundary cannot share.
    assert!(
        source_exit_copies(&function, &[], &row, declared, 4, 2)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        source_exit_copies(&function, &[references[0]], &row, declared, 4, 2),
        Err(FixedViewCopyError::UnsupportedSharedTransitionSet { function: 0 })
    );
    // A repeated member site is not a canonical destination set.
    let duplicated = [references[0], references[1], references[0]];
    assert_eq!(
        source_exit_copies(&function, &duplicated, &row, declared, 4, 2),
        Err(FixedViewCopyError::NonCanonicalCopies)
    );
}

/// The default-path leg emits the same dominating copy over the canonical
/// fan-out — one copy at the shared exit instead of one per fixed use.
#[test]
fn shared_source_exit_policy_shares_one_copy_across_the_fan_out() {
    let (function, legality, row) = fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, transformed) = source_exit_copies_transformed(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
        4,
        2,
    )
    .unwrap();
    assert_eq!(copies.len(), 1);
    let copy = &copies[0];
    assert_eq!(copy.insertion_block, SelectedBlockId(0));
    assert_eq!(copy.before_instruction, SelectedInstructionId(1));
    assert_eq!(
        copy.destinations
            .iter()
            .map(|destination| destination.block)
            .collect::<Vec<_>>(),
        vec![SelectedBlockId(1), SelectedBlockId(2)]
    );
    for leaf in &transformed.blocks[1..] {
        let SelectedTerminator::Return { instruction, .. } = &leaf.terminator else {
            panic!()
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            VirtualRegisterId(2)
        );
    }
}

/// A boundary whose fragment entered without connector evidence cannot
/// share the exit: the default leg falls back to a copy in each site's own
/// block while the declared selection refuses.
#[test]
fn unconnected_boundaries_fall_back_to_site_copies_or_refuse_under_the_declared_leg() {
    let (function, legality, row) = fixture();
    let mut boundaries = boundaries(&legality);
    boundaries[0].incoming = None;
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, transformed) = source_exit_copies_transformed(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
        4,
        2,
    )
    .unwrap();
    assert_eq!(copies.len(), 2);
    assert_eq!(copies[0].insertion_block, SelectedBlockId(1));
    assert_eq!(copies[0].before_instruction, SelectedInstructionId(2));
    assert_eq!(copies[1].insertion_block, SelectedBlockId(2));
    assert_eq!(copies[1].before_instruction, SelectedInstructionId(3));
    for (leaf, copy) in transformed.blocks[1..].iter().zip(&copies) {
        assert_eq!(leaf.instructions.len(), 1);
        assert_eq!(leaf.instructions[0].kind, SelectedInstructionKind::CopyI64);
        let SelectedTerminator::Return { instruction, .. } = &leaf.terminator else {
            panic!("leaf still returns")
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            copy.result_virtual_register
        );
    }
    assert_eq!(
        source_exit_copies(
            &function,
            &references,
            &row,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            4,
            2,
        ),
        Err(FixedViewCopyError::UnsupportedSharedTransitionSet { function: 0 })
    );
}

/// The default leg keeps the immediate form's origin admission: an
/// instruction-result source with mid-block sites admits — under the
/// declared selection the same unsharable boundary set refuses first.
#[test]
fn shared_source_exit_policy_admits_any_scalar_origin_like_the_immediate_form() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let (copies, transformed) = source_exit_copies_transformed(
        &function,
        &references,
        &row,
        FixedViewCopyPolicy::SharedSourceExitBeforeFixedUseV1,
        5,
        1,
    )
    .unwrap();
    assert_eq!(copies.len(), 3);
    assert_eq!(
        copies
            .iter()
            .map(|copy| copy.before_instruction)
            .collect::<Vec<_>>(),
        vec![
            SelectedInstructionId(1),
            SelectedInstructionId(2),
            SelectedInstructionId(4)
        ]
    );
    let block0 = &transformed.blocks[0];
    assert_eq!(
        block0.instructions[2].operands[0].virtual_register,
        VirtualRegisterId(1)
    );
    assert_eq!(
        block0.instructions[4].operands[0].virtual_register,
        VirtualRegisterId(2)
    );
    assert_eq!(
        source_exit_copies(
            &function,
            &references,
            &row,
            FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1,
            5,
            1,
        ),
        Err(FixedViewCopyError::UnsupportedSharedTransitionSet { function: 0 })
    );
}

/// An instruction-result register consumed at three incompatible fixed-use
/// operand sites — two ordinary instructions in the entry block and a return
/// terminator in the successor — with chained from-views (each site sources
/// the previous pin), the shape the immediate-site policy exists to admit.
pub(crate) fn immediate_fixture() -> (
    SelectedFunction,
    Vec<super::super::evidence::AuthenticatedFixedViewBoundary>,
    RegisterInstructionConstraint,
) {
    let machine = MachineId::new(1).unwrap();
    let class = RegisterClassId(0);
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let source_value = ValueId::new(2).unwrap();
    let producer = instruction(
        0,
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(7),
        },
        Vec::new(),
    );
    let site = |instruction: u32| VirtualFixedConstraintSite::Operand {
        position: LivenessPosition(instruction),
        point: LiveRangePoint(instruction),
        instruction: SelectedInstructionId(instruction),
        operand: 0,
        access: RegisterOperandAccess::Use,
    };
    let function = SelectedFunction {
        machine,
        attachment: None,
        provenance: Default::default(),
        structural: None,
        outgoing_arguments: Vec::new(),
        local_storage_slots: Vec::new(),
        calls: Vec::new(),
        normalized_foreign_calls: Vec::new(),
        memory_accesses: Vec::new(),
        boundary_settlements: Vec::new(),
        entry_block: SelectedBlockId(0),
        virtual_registers: vec![VirtualRegister {
            id: VirtualRegisterId(0),
            scalar_type: scalar,
            class,
            origin: VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(0),
                source_value,
            },
            definition_site: Some(ValueDefinitionSite::Node {
                block: BlockId::new(1).unwrap(),
                node: 0,
            }),
            entry_fixed_view: None,
        }],
        blocks: vec![
            SelectedBlock {
                id: SelectedBlockId(0),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(1).unwrap(),
                ),
                instructions: vec![
                    producer,
                    instruction(
                        1,
                        SelectedInstructionKind::CallScalar {
                            callee: MachineId::new(2).unwrap(),
                        },
                        vec![use_operand(0, class, Some(RegisterViewId(2)))],
                    ),
                    instruction(
                        2,
                        SelectedInstructionKind::CompareI64Zero,
                        vec![use_operand(0, class, Some(RegisterViewId(3)))],
                    ),
                ],
                terminator: SelectedTerminator::Jump {
                    instruction: instruction(3, SelectedInstructionKind::Jump, Vec::new()),
                    successor: SelectedSuccessor {
                        role: selected_instructions::SelectedSuccessorRole::Semantic,
                        structural_case: None,
                        structural_bindings: Vec::new(),
                        psi_edge: EdgeId::new(1).unwrap(),
                        block: SelectedBlockId(1),
                        source_target: BlockId::new(2).unwrap(),
                        bindings: Vec::new(),
                        fuel: Vec::new(),
                    },
                },
            },
            SelectedBlock {
                id: SelectedBlockId(1),
                origin: selected_instructions::SelectedBlockOrigin::Source(
                    BlockId::new(2).unwrap(),
                ),
                instructions: Vec::new(),
                terminator: SelectedTerminator::Return {
                    instruction: instruction(
                        4,
                        SelectedInstructionKind::ReturnScalar,
                        vec![use_operand(0, class, Some(RegisterViewId(4)))],
                    ),
                    psi_return_edge: EdgeId::new(2).unwrap(),
                },
            },
        ],
    };
    // Each site's from-view is the previous pin, so the third boundary could
    // never have sourced the register's (absent) entry fixed view.
    let sites: [(u32, u16, SelectedBlockId); 3] = [
        (1, 2, SelectedBlockId(0)),
        (2, 3, SelectedBlockId(0)),
        (4, 4, SelectedBlockId(1)),
    ];
    let boundaries = sites
        .into_iter()
        .enumerate()
        .map(|(index, (site_instruction, to_view, block))| {
            let index = index as u32;
            super::super::evidence::AuthenticatedFixedViewBoundary {
                function: 0,
                machine,
                virtual_register: VirtualRegisterId(0),
                class,
                source_segment: FixedPrecoloredSourceSegmentId(index),
                source_domain: FixedPrecoloredHomeDomainId(index),
                from_view: RegisterViewId(index as u16 + 1),
                destination_segment: FixedPrecoloredSourceSegmentId(index + 3),
                destination_domain: FixedPrecoloredHomeDomainId(index + 3),
                site: site(site_instruction),
                block,
                to_view: RegisterViewId(to_view),
                incoming: None,
            }
        })
        .collect();
    let row = RegisterInstructionConstraint {
        id: RegisterConstraintId(9),
        key: key(9),
        operands: vec![
            RegisterOperandConstraint {
                operand: 0,
                access: RegisterOperandAccess::Use,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
            RegisterOperandConstraint {
                operand: 1,
                access: RegisterOperandAccess::Def,
                class,
                fixed_view: None,
                tied_to: None,
                early_clobber: false,
            },
        ],
        implicit_uses: Vec::new(),
        implicit_defs: Vec::new(),
        clobbers: Vec::new(),
    };
    (function, boundaries, row)
}

/// The immediate-site policy drops one copy into each boundary's own block
/// immediately before the fixed-use instruction — an ordinary site takes the
/// site's position, a terminator site appends to the block — and retargets
/// only that site's operand onto the fresh segment register.
#[test]
fn immediate_site_policy_places_copies_before_each_fixed_use_site() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    let copies = super::site::build_site_copies(
        0,
        &function,
        &references,
        &mut transformed,
        &row,
        row.key,
        false,
        5,
        1,
    )
    .unwrap();
    assert_eq!(copies.len(), 3);
    assert_eq!(
        copies
            .iter()
            .map(|copy| copy.before_instruction)
            .collect::<Vec<_>>(),
        vec![
            SelectedInstructionId(1),
            SelectedInstructionId(2),
            SelectedInstructionId(4)
        ]
    );
    assert_eq!(
        copies
            .iter()
            .map(|copy| copy.insertion_block)
            .collect::<Vec<_>>(),
        vec![SelectedBlockId(0), SelectedBlockId(0), SelectedBlockId(1)]
    );
    let block0 = &transformed.blocks[0];
    assert_eq!(
        block0
            .instructions
            .iter()
            .map(|instruction| instruction.kind)
            .collect::<Vec<_>>(),
        vec![
            SelectedInstructionKind::MaterializeI64 {
                value: IntegerValue::Unsigned(7)
            },
            SelectedInstructionKind::CopyI64,
            SelectedInstructionKind::CallScalar {
                callee: MachineId::new(2).unwrap()
            },
            SelectedInstructionKind::CopyI64,
            SelectedInstructionKind::CompareI64Zero,
        ]
    );
    assert_eq!(
        block0.instructions[2].operands[0].virtual_register,
        VirtualRegisterId(1)
    );
    assert_eq!(
        block0.instructions[4].operands[0].virtual_register,
        VirtualRegisterId(2)
    );
    let block1 = &transformed.blocks[1];
    assert_eq!(block1.instructions.len(), 1);
    assert_eq!(
        block1.instructions[0].kind,
        SelectedInstructionKind::CopyI64
    );
    let SelectedTerminator::Return { instruction, .. } = &block1.terminator else {
        panic!("successor still returns")
    };
    assert_eq!(
        instruction.operands[0].virtual_register,
        VirtualRegisterId(3)
    );
    assert_eq!(transformed.virtual_registers.len(), 4);
    for (segment, copy) in copies.iter().enumerate() {
        let register = &transformed.virtual_registers[segment + 1];
        assert_eq!(register.id, copy.result_virtual_register);
        assert_eq!(register.entry_fixed_view, None);
        assert!(matches!(
            register.origin,
            VirtualRegisterOrigin::InstructionResult { instruction, .. }
                if instruction == copy.copy_instruction
        ));
    }
}

/// The immediate form carries no live-in gate: an `InstructionResult` source
/// with no entry fixed view, and chained boundaries whose from-views are
/// earlier pins, admit — while the leaf-local V1 gate still refuses both.
#[test]
fn immediate_site_policy_admits_origins_and_chained_sources_leaf_local_refuses() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let mut refused = function.clone();
    assert!(matches!(
        super::site::build_site_copies(
            0,
            &function,
            &references,
            &mut refused,
            &row,
            row.key,
            true,
            5,
            1,
        ),
        Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: 0,
            register: 0
        })
    ));
    let mut transformed = function.clone();
    assert_eq!(
        super::site::build_site_copies(
            0,
            &function,
            &references,
            &mut transformed,
            &row,
            row.key,
            false,
            5,
            1,
        )
        .unwrap()
        .len(),
        3
    );

    // An entry parameter still carries its live-in pin; a boundary sourcing
    // a different pinned view is admitted only by the immediate form.
    let mut entered = function.clone();
    entered.virtual_registers[0].origin = VirtualRegisterOrigin::EntryParameter {
        source_value: ValueId::new(2).unwrap(),
        parameter_index: 0,
    };
    entered.virtual_registers[0].entry_fixed_view = Some(RegisterViewId(9));
    let mut refused = entered.clone();
    assert!(matches!(
        super::site::build_site_copies(
            0,
            &entered,
            &references,
            &mut refused,
            &row,
            row.key,
            true,
            5,
            1,
        ),
        Err(FixedViewCopyError::UnsupportedSourceRegister {
            function: 0,
            register: 0
        })
    ));
    let mut transformed = entered.clone();
    assert_eq!(
        super::site::build_site_copies(
            0,
            &entered,
            &references,
            &mut transformed,
            &row,
            row.key,
            false,
            5,
            1,
        )
        .unwrap()
        .len(),
        3
    );
}

/// Moved split points, unauthenticated sites, duplicate destinations, and
/// out-of-admission sources all reject before any copy is materialized.
#[test]
fn immediate_site_copy_rejects_malformed_boundary_and_source_premises() {
    let (function, boundaries, row) = immediate_fixture();
    let build = |function: &SelectedFunction,
                 boundaries: &[super::super::evidence::AuthenticatedFixedViewBoundary],
                 transformed: &mut SelectedFunction| {
        let references = boundaries.iter().collect::<Vec<_>>();
        super::site::build_site_copies(
            0,
            function,
            &references,
            transformed,
            &row,
            row.key,
            false,
            5,
            1,
        )
    };

    // A boundary moved onto the wrong block is not the site's own block.
    let mut moved = boundaries.clone();
    moved[0].block = SelectedBlockId(1);
    let mut transformed = function.clone();
    assert!(matches!(
        build(&function, &moved, &mut transformed),
        Err(FixedViewCopyError::SegmentEvidenceMismatch)
    ));

    // A non-Use site can never be a fixed-use boundary site.
    let mut defined = boundaries.clone();
    let VirtualFixedConstraintSite::Operand { access, .. } = &mut defined[0].site else {
        unreachable!()
    };
    *access = RegisterOperandAccess::UseDef;
    let mut transformed = function.clone();
    assert!(matches!(
        build(&function, &defined, &mut transformed),
        Err(FixedViewCopyError::UnsupportedTransitionSite {
            function: 0,
            register: 0
        })
    ));

    // Two boundaries on the same operand site cannot produce two copies.
    let duplicated = vec![boundaries[0], boundaries[0]];
    let mut transformed = function.clone();
    assert!(matches!(
        build(&function, &duplicated, &mut transformed),
        Err(FixedViewCopyError::NonCanonicalCopies)
    ));

    // A destination view the operand does not actually pin is not the
    // recorded boundary's destination.
    let mut retargeted = boundaries.clone();
    retargeted[0].to_view = RegisterViewId(99);
    let mut transformed = function.clone();
    assert!(matches!(
        build(&function, &retargeted, &mut transformed),
        Err(FixedViewCopyError::MissingDestination {
            function: 0,
            instruction: 1
        })
    ));

    // Origins that never named a scalar source value stay out of admission.
    for origin in [
        VirtualRegisterOrigin::InstructionScratch {
            instruction: SelectedInstructionId(0),
            operand: 0,
        },
        VirtualRegisterOrigin::AbiTransport {
            instruction: SelectedInstructionId(0),
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            byte_offset: 0,
        },
    ] {
        let mut changed = function.clone();
        changed.virtual_registers[0].origin = origin;
        let mut transformed = changed.clone();
        assert!(matches!(
            build(&changed, &boundaries, &mut transformed),
            Err(FixedViewCopyError::UnsupportedSourceRegister {
                function: 0,
                register: 0
            })
        ));
    }
}

/// Two derivations over the identical boundary set produce the identical
/// copies and transformed function — including the site positions each copy
/// landed at.
#[test]
fn immediate_site_copies_are_deterministic() {
    let (function, boundaries, row) = immediate_fixture();
    let references = boundaries.iter().collect::<Vec<_>>();
    let mut transformed_a = function.clone();
    let mut transformed_b = function.clone();
    let first = super::site::build_site_copies(
        0,
        &function,
        &references,
        &mut transformed_a,
        &row,
        row.key,
        false,
        5,
        1,
    )
    .unwrap();
    let second = super::site::build_site_copies(
        0,
        &function,
        &references,
        &mut transformed_b,
        &row,
        row.key,
        false,
        5,
        1,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(transformed_a, transformed_b);
}

/// The V1 leaf-local gate remains selectable: an entry-pinned source with
/// return-terminator sites still places one copy per leaf.
#[test]
fn leaf_local_policy_still_places_one_copy_per_return_leaf() {
    let (function, legality, row) = fixture();
    let boundaries = boundaries(&legality);
    let references = boundaries.iter().collect::<Vec<_>>();
    let mut transformed = function.clone();
    let copies = super::site::build_site_copies(
        0,
        &function,
        &references,
        &mut transformed,
        &row,
        row.key,
        true,
        4,
        2,
    )
    .unwrap();
    assert_eq!(copies.len(), 2);
    for (leaf, copy) in transformed.blocks[1..].iter().zip(&copies) {
        assert_eq!(leaf.instructions.len(), 1);
        assert_eq!(leaf.instructions[0].kind, SelectedInstructionKind::CopyI64);
        let SelectedTerminator::Return { instruction, .. } = &leaf.terminator else {
            panic!("leaf still returns")
        };
        assert_eq!(
            instruction.operands[0].virtual_register,
            copy.result_virtual_register
        );
        assert_eq!(copy.insertion_block, leaf.id);
    }
}
