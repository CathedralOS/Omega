//! Tests selection for sum rows the target plan already stores.
//!
//! Each scenario builds the target row directly, prepares it with stage 04's
//! fixture builders and runs selection. Selection no longer removes copies and
//! dead stores: that cleanup is part of the opt-in Stage 06 rewrite
//! `SelectedPackedVectorPromotionV1`. The row carries the tag, offset, layout,
//! pointer chase, and referent-region size that stage 02 records. The expected
//! text pins the selected opcodes, tags, sizes and memory accesses for that
//! geometry.

use abstract_operations::{
    AbstractOperation, AbstractOperationPlan, AbstractStructuralCaseSuccessor,
};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
};
use semantic_vocabulary::{
    IntegerValue, ScalarType, StructuralFieldId, StructuralTypeId, WorkScheduleIdentity,
};
use target_lowering::test_support::*;
use target_operations::{
    TargetCasePointerSlot, TargetControlCaseSuccessor, TargetControlTerminator,
    TargetOperationPlan, TargetStructuralCaseSource, TargetStructuralHomeLayout,
    TargetUnitOperation, TargetUnitScalarArgumentSource,
};
use target_operations_to_selected_instructions::test_support as selection;
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalIdentityTableBuilder, TerminalModule,
};
use verified_optimization_input::publication::target_replay::BoundaryOriginFacts;

#[test]
fn stored_sum_rows_select_the_same_instructions_as_main() {
    let dump = [
        tag_only(),
        payload_behind_pointer(),
        tag_only_behind_pointer(),
        sum_inside_record(),
        membership_behind_pointer(),
        establish_case(),
        activation_claim(),
    ]
    .join("");
    let expected = include_str!("sum_layout_selection.expected.txt");
    assert_eq!(dump, expected, "selected opcodes, offsets, tags, and sizes");
}

fn tag_only() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("TagOnly"), &mut names, &[]);
    dispatch("tag-only", names, vec![sum], ty(1), Vec::new())
}

fn payload_behind_pointer() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let byte = names.intern("byte");
    let sum = sum_type(
        1,
        names.intern("Payload"),
        &mut names,
        &[scalar_field(1, byte, u32_type())],
    );
    let reference = reference_type(2, names.intern("PayloadRef"), ty(1));
    dispatch(
        "payload-behind-pointer",
        names,
        vec![sum, reference],
        ty(2),
        vec![StructuralPathSegment::Referent],
    )
}

fn tag_only_behind_pointer() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("Pointed"), &mut names, &[]);
    let reference = reference_type(2, names.intern("PointedRef"), ty(1));
    dispatch(
        "tag-only-behind-pointer",
        names,
        vec![sum, reference],
        ty(2),
        vec![StructuralPathSegment::Referent],
    )
}

fn sum_inside_record() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let prefix = names.intern("prefix");
    let body = names.intern("body");
    let sum = sum_type(1, names.intern("Inner"), &mut names, &[]);
    let record = StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id: ty(2),
        identity: names.intern("Outer"),
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape: StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![
                scalar_field(1, prefix, u32_type()),
                StructuralFieldDeclaration {
                    declaration_identity: Default::default(),
                    inline_carrier: None,
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: body,
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(ty(1)),
                    storage: None,
                },
            ],
        },
        scalar_restriction: None,
        requires: Vec::new(),
    };
    dispatch(
        "sum-inside-record",
        names,
        vec![sum, record],
        ty(2),
        vec![StructuralPathSegment::Field(body)],
    )
}

fn membership_behind_pointer() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let payload = names.intern("payload");
    let sum = sum_type(
        1,
        names.intern("Member"),
        &mut names,
        &[scalar_field(1, payload, u32_type())],
    );
    let reference = reference_type(2, names.intern("MemberRef"), ty(1));
    let plan = plan(
        names,
        vec![sum, reference],
        function_with_body(
            vec![parameter(ty(2))],
            vec![entry(
                1,
                vec![
                    AbstractOperation::StructuralCaseMembership {
                        psi_operation: op(1),
                        result: boolean_result(),
                        source: place(1),
                        path: vec![StructuralPathSegment::Referent],
                        case: case_id(1),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let geometry = geometry(&plan, ty(2), &[StructuralPathSegment::Referent]);
    assert_eq!(geometry.pointer_slots.len(), 1);
    let membership = TargetUnitOperation::StructuralCaseMembership {
        psi_operation: op(1),
        result: boolean_result(),
        source: place(1),
        path: vec![StructuralPathSegment::Referent],
        tag_byte_offset: 0,
        case: case_id(1),
        case_tag: 0,
        indices: Vec::new(),
        pointer_slots: carried_slots(&geometry),
    };
    let target = target_plan(&plan, vec![block(1, vec![membership], return_unit(1))]);
    select("membership-behind-pointer", &plan, target)
}

fn establish_case() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let sum = sum_type(1, names.intern("Established"), &mut names, &[]);
    let result = structural_result(place(1), ty(1));
    let plan = plan(
        names,
        vec![sum],
        function_with_body(
            Vec::new(),
            vec![entry(
                1,
                vec![
                    AbstractOperation::EstablishCase {
                        psi_operation: op(1),
                        result: result.clone(),
                        result_case: case_id(1),
                        fields: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let layout = sum_layout(&plan, ty(1));
    let target = target_plan(
        &plan,
        vec![block(
            1,
            vec![TargetUnitOperation::EstablishCase {
                psi_operation: op(1),
                result_home: home(op(1), &result, TargetStructuralHomeLayout::Sum(layout)),
                result_case: case_id(1),
                fields: Vec::new(),
            }],
            return_unit(1),
        )],
    );
    select("establish-case", &plan, target)
}

fn activation_claim() -> String {
    let mut names = TerminalIdentityTableBuilder::default();
    let bound = names.intern("bound");
    let sum = sum_type(
        1,
        names.intern("Claim"),
        &mut names,
        &[scalar_field(1, bound, u64_type())],
    );
    let result = structural_result(place(1), ty(1));
    let integer = u64_type();
    let plan = plan(
        names,
        vec![sum],
        function_with_body(
            vec![parameter_at(2, ty(1))],
            vec![entry(
                1,
                vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: op(1),
                        result: val(1),
                        scalar_type: ScalarType::Integer(integer),
                        value: IntegerValue::Unsigned(4),
                    },
                    AbstractOperation::EstablishActivationClaim {
                        psi_operation: op(2),
                        result: result.clone(),
                        bound: 8,
                        committed: val(1),
                        backing_type: ty(1),
                        established_case: case_id(1),
                        rejected_case: case_id(2),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: edge(1),
                        cleanup_actions: Vec::new(),
                    },
                ],
            )],
        ),
    );
    let layout = sum_layout(&plan, ty(1));
    let claim = TargetUnitOperation::EstablishActivationClaim {
        psi_operation: op(2),
        result_home: home(op(2), &result, TargetStructuralHomeLayout::Sum(layout)),
        bound: 8,
        committed: TargetUnitScalarArgumentSource::IntegerImmediate {
            defining_operation: op(1),
            source_value: val(1),
            scalar_type: integer,
            value: IntegerValue::Unsigned(4),
        },
        backing_type: ty(1),
        established_case: case_id(1),
        rejected_case: case_id(2),
    };
    let constant = TargetUnitOperation::IntegerConstant {
        psi_operation: op(1),
        result: val(1),
        scalar_type: integer,
        value: IntegerValue::Unsigned(4),
    };
    let target = target_plan(&plan, vec![block(1, vec![constant, claim], return_unit(1))]);
    select("activation-claim", &plan, target)
}

fn dispatch(
    name: &str,
    names: TerminalIdentityTableBuilder,
    types: Vec<StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
) -> String {
    let arms = vec![
        AbstractStructuralCaseSuccessor {
            psi_edge: edge(1),
            target: block_id(2),
            case: case_id(1),
            payloads: Vec::new(),
            structural_bindings: Vec::new(),
            cleanup_actions: Vec::new(),
        },
        AbstractStructuralCaseSuccessor {
            psi_edge: edge(2),
            target: block_id(3),
            case: case_id(2),
            payloads: Vec::new(),
            structural_bindings: Vec::new(),
            cleanup_actions: Vec::new(),
        },
    ];
    let plan = plan(
        names,
        types,
        function_with_body(
            vec![parameter(root)],
            vec![
                entry(
                    1,
                    vec![AbstractOperation::StructuralCase {
                        psi_block: block_id(1),
                        source: place(1),
                        path: path.clone(),
                        cases: arms.clone(),
                    }],
                ),
                entry(
                    2,
                    vec![AbstractOperation::ReturnUnit {
                        psi_edge: edge(3),
                        cleanup_actions: Vec::new(),
                    }],
                ),
                entry(
                    3,
                    vec![AbstractOperation::ReturnUnit {
                        psi_edge: edge(4),
                        cleanup_actions: Vec::new(),
                    }],
                ),
            ],
        ),
    );
    let geometry = geometry(&plan, root, &path);
    let selected = sum_layout(&plan, geometry.selected);
    let layouts =
        legalized_operations::native_geometry::NativeGeometry::new(&plan.structural_types, HOST())
            .unwrap();
    let root_declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == root)
        .unwrap_or_else(|| panic!("{name}: missing root declaration"));
    let root_layout = if matches!(
        root_declaration.shape,
        terminal_psi::sum_or_mixed_shape_pattern!()
    ) {
        TargetStructuralHomeLayout::Sum(
            layouts
                .sum_layout(root)
                .unwrap_or_else(|error| panic!("{name}: root sum layout {error:?}")),
        )
    } else {
        TargetStructuralHomeLayout::Aggregate(
            legalized_operations::structural_inputs::shape(root, &layouts)
                .unwrap_or_else(|| panic!("{name}: root aggregate shape")),
        )
    };
    let source = TargetStructuralCaseSource::BorrowedParameter {
        parameter: abi_parameter(&plan, &plan.functions[0].structural_parameters[0]),
        layout: root_layout,
    };
    let terminator = TargetControlTerminator::StructuralCase {
        psi_block: block_id(1),
        indices: Vec::new(),
        source,
        path: path.clone(),
        byte_offset: geometry.byte_offset,
        layout: selected.clone(),
        pointer_slots: carried_slots(&geometry),
        region_byte_size: carried_region_size(&plan, &geometry, selected.shape.byte_size),
        cases: arms
            .iter()
            .enumerate()
            .map(|(ordinal, arm)| TargetControlCaseSuccessor {
                psi_edge: arm.psi_edge,
                case: arm.case,
                case_tag: i32::try_from(ordinal).unwrap(),
                target: arm.target,
                payloads: Vec::new(),
                structural_bindings: Vec::new(),
                structural_indices: Vec::new(),
                work: Vec::new(),
                cleanup_actions: Vec::new(),
            })
            .collect(),
    };
    let target = target_plan(
        &plan,
        vec![
            block(1, Vec::new(), terminator),
            block(2, Vec::new(), return_unit(3)),
            block(3, Vec::new(), return_unit(4)),
        ],
    );
    select(name, &plan, target)
}

fn select(name: &str, plan: &AbstractOperationPlan, target: TargetOperationPlan) -> String {
    let unit = reconstruct_psi_optimization_unit_seed(plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap_or_else(|error| panic!("{name}: unit {error:?}"));
    let module = TerminalModule::for_entry(plan.entry);
    let interrupts = provider_admission::AdmittedInstalledInterruptCohort::empty(plan.psi);
    let callable_program =
        optimization_unit_semantics::callable_values::optimized_callable_values(&unit)
            .unwrap_or_else(|reason| panic!("{name}: callable projection {reason}"));
    assert!(
        callable_program
            .bodies
            .iter()
            .all(|(_, body)| body.invocations.is_empty()),
        "{name}: the sum-layout fixture has no call occurrences"
    );
    let callable_bindings = verified_optimization_input::NativeCallableBindings::default();
    let prepared = prepare_fixture_execution_plan(
        &target,
        &unit,
        BoundaryOriginFacts {
            program_entry: None,
            target_context: None,
            module: &module,
        },
        &callable_bindings,
        &interrupts,
        fixture_types(),
    )
    .unwrap_or_else(|error| panic!("{name}: prepare {error:?}"));
    // From here selection sees the prepared plan and nothing behind it.
    let selected = selection::select_instructions_for_plan(
        &prepared,
        &selection::BoundInstallations::empty(interrupts.psi()),
    )
    .unwrap_or_else(|error| panic!("{name}: select {error:?}"));
    let mut text = format!("== {name}\n");
    for function in selected.plan().functions.iter() {
        for slot in &function.local_storage_slots {
            text.push_str(&format!(
                "slot size={} align={}\n",
                slot.byte_size, slot.alignment
            ));
        }
        for block in function.blocks.iter() {
            text.push_str(&format!("block {:?} {:?}\n", block.id, block.origin));
            for instruction in &block.instructions {
                text.push_str(&format!("  {}\n", opcode(instruction)));
            }
            text.push_str(&format!(
                "  term {}\n",
                opcode(term_instruction(&block.terminator))
            ));
            for successor in term_successors(&block.terminator) {
                if let Some(case) = &successor.structural_case {
                    text.push_str(&format!(
                        "  case tag={} size={}\n",
                        case.case_tag,
                        case.source
                            .byte_size()
                            .map(|size| size.to_string())
                            .unwrap_or_else(|| "-".to_owned())
                    ));
                }
            }
        }
        for access in &function.memory_accesses {
            text.push_str(&format!(
                "access offset={} count={}\n",
                access.byte_offset, access.byte_count
            ));
        }
    }
    text
}

fn opcode(instruction: &SelectedInstruction) -> String {
    match instruction.kind {
        SelectedInstructionKind::MaterializeI64 { value } => format!("MaterializeI64 {value:?}"),
        other => format!("{other:?}"),
    }
}

fn term_instruction(terminator: &SelectedTerminator) -> &SelectedInstruction {
    match terminator {
        SelectedTerminator::EliminateEmpty { instruction, .. }
        | SelectedTerminator::InvokeNoNormal { instruction, .. }
        | SelectedTerminator::OperationInvokeNoNormal { instruction, .. }
        | SelectedTerminator::OperationTrap { instruction, .. }
        | SelectedTerminator::Crash { instruction, .. }
        | SelectedTerminator::HostedExitProcess { instruction, .. }
        | SelectedTerminator::Jump { instruction, .. }
        | SelectedTerminator::ConditionalBranch { instruction, .. }
        | SelectedTerminator::ConditionalBranchU64LessThan { instruction, .. }
        | SelectedTerminator::ConditionalBranchI64LessThan { instruction, .. }
        | SelectedTerminator::Return { instruction, .. } => instruction,
    }
}

fn term_successors(terminator: &SelectedTerminator) -> Vec<&SelectedSuccessor> {
    match terminator {
        SelectedTerminator::Jump { successor, .. } => vec![successor],
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchU64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less: when_nonzero,
            when_not_less: when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        _ => Vec::new(),
    }
}

fn carried_slots(
    geometry: &legalized_operations::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
) -> Vec<TargetCasePointerSlot> {
    geometry
        .pointer_slots
        .iter()
        .map(|slot| TargetCasePointerSlot {
            byte_offset: slot.byte_offset,
            index_start: u32::try_from(slot.indices.start).expect("pointer index"),
            index_end: u32::try_from(slot.indices.end).expect("pointer index"),
            scalar_descriptor: slot.scalar_descriptor,
        })
        .collect()
}

fn carried_region_size(
    plan: &AbstractOperationPlan,
    geometry: &legalized_operations::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
    sum_size: u32,
) -> u32 {
    if geometry.pointer_slots.is_empty() && geometry.region_root == geometry.selected {
        sum_size
    } else {
        let layouts = legalized_operations::native_geometry::NativeGeometry::new(
            &plan.structural_types,
            HOST(),
        )
        .unwrap();
        legalized_operations::structural_inputs::structural_reference_input::shape(
            geometry.region_root,
            &layouts,
        )
        .expect("region size")
        .byte_size
    }
}

fn geometry(
    plan: &AbstractOperationPlan,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> legalized_operations::structural_inputs::structural_reference_input::ReferenceSourceGeometry {
    let layouts =
        legalized_operations::native_geometry::NativeGeometry::new(&plan.structural_types, HOST())
            .unwrap();
    legalized_operations::structural_inputs::structural_reference_input::reference_source_geometry(
        root, path, &layouts,
    )
    .expect("sum geometry")
}

fn sum_layout(
    plan: &AbstractOperationPlan,
    sum: StructuralTypeId,
) -> calling_conventions::ConventionalSumLayout {
    let layouts =
        legalized_operations::native_geometry::NativeGeometry::new(&plan.structural_types, HOST())
            .unwrap();
    layouts.sum_layout(sum).expect("sum layout")
}
