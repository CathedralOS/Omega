//! Selected instructions for sum rows the target plan already stores.
//!
//! Each scenario builds the target row directly and runs legalization and
//! selection. The row carries the tag, offset, layout, pointer chase, and
//! referent-region size that stage 02 records. The expected text is the
//! selection `origin/main` emitted for that same geometry.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractResult, AbstractStructuralCaseSuccessor,
};
use calling_conventions::{CallSignature, CallingPolicy};
use optimization_unit::reconstruct_psi_optimization_unit_seed;
use selected_instructions::{
    SelectedInstruction, SelectedInstructionKind, SelectedSuccessor, SelectedTerminator,
};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, PlaceId,
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
    WorkScheduleIdentity,
};
use target::NativeTarget;
use target_operations::{
    BoundaryOriginFacts, TargetCasePointerSlot, TargetControlBlock, TargetControlCaseSuccessor,
    TargetControlGraph, TargetControlTerminator, TargetFunction, TargetOperationPlan,
    TargetStructuralCaseSource, TargetStructuralHomeLayout, TargetStructuralHomeOrigin,
    TargetStructuralHomeRequirement, TargetStructuralParameter, TargetUnitOperation,
    TargetUnitScalarArgumentSource, TerminalPsiProvenance,
};
use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape, TerminalIdentityIndex,
    TerminalIdentityTableBuilder, TerminalModule, TerminalPsiIdentity, VocabularyMarker,
};

const HOST: fn() -> NativeTarget = NativeTarget::linux_x64;

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
        quotient_carrier: None,
        id: ty(2),
        identity: names.intern("Outer"),
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        shape: StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![
                scalar_field(1, prefix, u32_type()),
                StructuralFieldDeclaration {
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
            vec![entry(1, 0)],
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
            vec![entry(1, 0)],
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
            vec![entry(1, 0)],
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
                AbstractOperation::StructuralCase {
                    source: place(1),
                    path: path.clone(),
                    cases: arms.clone(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(3),
                    cleanup_actions: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge(4),
                    cleanup_actions: Vec::new(),
                },
            ],
            vec![entry(1, 0), entry(2, 1), entry(3, 2)],
        ),
    );
    let geometry = geometry(&plan, root, &path);
    let selected = sum_layout(&plan, geometry.selected);
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let prepared = crate::native_geometry::NativeInput::new(&plan, &layouts);
    let root_layout =
        crate::legalization_scalar_graph::aggregate_results::type_home_layout(root, &prepared)
            .unwrap_or_else(|error| panic!("{name}: root layout {error:?}"));
    let source = TargetStructuralCaseSource::BorrowedParameter {
        parameter: abi_parameter(&plan, &plan.functions[0].structural_parameters[0]),
        layout: root_layout,
    };
    let terminator = TargetControlTerminator::StructuralCase {
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

fn validate_stored_geometry(
    target: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<(), crate::legalization::LegalizationError> {
    use crate::legalization_scalar_graph::structural_case::{
        dispatch_projection, membership_layout, source_result,
    };
    // These hand-built selection rows omit unrelated provenance metadata. Test
    // the validator's geometry checks directly, with the same rows selection uses.
    let geometry = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan = &crate::native_geometry::NativeInput::new(plan, &geometry);
    let function = &unit.functions[0];
    for block in &target.functions[0].graph.blocks {
        if let TargetControlTerminator::StructuralCase {
            source,
            path,
            pointer_slots,
            region_byte_size,
            ..
        } = &block.terminator
        {
            dispatch_projection(
                function,
                source.place(),
                path,
                plan,
                pointer_slots,
                *region_byte_size,
            )?;
        }
        for operation in &block.operations {
            if let TargetUnitOperation::StructuralCaseMembership {
                source,
                path,
                case,
                pointer_slots,
                ..
            } = operation
            {
                membership_layout(
                    function,
                    *source,
                    path,
                    *case,
                    plan,
                    source_result(function, *source),
                    pointer_slots,
                )?;
            }
        }
    }
    Ok(())
}

fn select(name: &str, plan: &AbstractOperationPlan, target: TargetOperationPlan) -> String {
    let unit = reconstruct_psi_optimization_unit_seed(plan, WorkScheduleIdentity::new(1).unwrap())
        .unwrap_or_else(|error| panic!("{name}: unit {error:?}"));
    // Selection trusts retained geometry; the independent validator must reject
    // a forged pointer chase or region size rather than ignore those fields.
    validate_stored_geometry(&target, plan, &unit)
        .unwrap_or_else(|error| panic!("{name}: target validation {error:?}"));
    for change_region_size in [false, true] {
        let mut forged = target.clone();
        let mut changed = false;
        for function in &mut forged.functions {
            for block in &mut function.graph.blocks {
                if let TargetControlTerminator::StructuralCase {
                    pointer_slots,
                    region_byte_size,
                    ..
                } = &mut block.terminator
                {
                    if change_region_size {
                        *region_byte_size += 1;
                    } else {
                        pointer_slots.push(TargetCasePointerSlot {
                            byte_offset: 0,
                            index_start: 0,
                            index_end: 0,
                            scalar_descriptor: false,
                        });
                    }
                    changed = true;
                }
                if !change_region_size {
                    for operation in &mut block.operations {
                        if let TargetUnitOperation::StructuralCaseMembership {
                            pointer_slots, ..
                        } = operation
                        {
                            pointer_slots.push(TargetCasePointerSlot {
                                byte_offset: 0,
                                index_start: 0,
                                index_end: 0,
                                scalar_descriptor: false,
                            });
                            changed = true;
                        }
                    }
                }
            }
        }
        if changed {
            assert!(
                validate_stored_geometry(&forged, plan, &unit).is_err(),
                "{name}: forged retained geometry accepted (region={change_region_size})"
            );
        }
    }
    let module = TerminalModule::for_entry(plan.entry);
    let interrupts = provider_admission::AdmittedInstalledInterruptCohort::empty(plan.psi);
    let callable_program =
        optimization_unit_semantics::callable_values::optimized_callable_values(&unit)
            .unwrap_or_else(|reason| panic!("{name}: callable projection {reason}"));
    assert!(
        callable_program
            .bodies
            .values()
            .all(|body| body.invocations.is_empty() && body.invalid_invocations.is_empty()),
        "{name}: the sum-layout fixture has no call occurrences"
    );
    let callable_roots = terminal_semantics::callable_values::CallableInvocationRoots::new();
    let legalized = crate::legalize_target_operations(
        &target,
        plan,
        &unit,
        BoundaryOriginFacts {
            program_entry: None,
            assumed_bindings: &[],
            target_machine_selections: None,
            module: &module,
        },
        &callable_roots,
        &interrupts,
        None,
        &[],
    )
    .unwrap_or_else(|error| panic!("{name}: legalize {error:?}"));
    let environment = register_environment::baseline_target_register_environment(target.target)
        .unwrap_or_else(|error| panic!("{name}: registers {error:?}"));
    let constraints =
        crate::optimized::constraints::selection_constraints(&legalized, &environment);
    let selected = crate::select_instructions(&legalized, &constraints, &environment)
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
        SelectedTerminator::InvokeNoNormal { instruction, .. }
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
    geometry: &crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
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
    geometry: &crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
    sum_size: u32,
) -> u32 {
    if geometry.pointer_slots.is_empty() && geometry.region_root == geometry.selected {
        sum_size
    } else {
        let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
        crate::structural_inputs::structural_reference_input::shape(geometry.region_root, &layouts)
            .expect("region size")
            .byte_size
    }
}

fn geometry(
    plan: &AbstractOperationPlan,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    crate::structural_inputs::structural_reference_input::reference_source_geometry(
        root, path, &layouts,
    )
    .expect("sum geometry")
}

fn sum_layout(
    plan: &AbstractOperationPlan,
    sum: StructuralTypeId,
) -> calling_conventions::ConventionalSumLayout {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let plan = crate::native_geometry::NativeInput::new(plan, &layouts);
    crate::legalization_scalar_graph::aggregate_results::sum_type_layout(sum, &plan)
        .expect("sum layout")
}

fn plan(
    names: TerminalIdentityTableBuilder,
    types: Vec<StructuralTypeDeclaration>,
    function: AbstractFunction,
) -> AbstractOperationPlan {
    AbstractOperationPlan {
        psi: psi(),
        entry: machine(),
        structural_types: types.into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        callables: Default::default(),
        dynamic_dispatch: Default::default(),
        identities: names.build(),
        functions: vec![function],
    }
}

fn function_with_body(
    structural_parameters: Vec<StructuralParameterDeclaration>,
    operations: Vec<AbstractOperation>,
    block_entries: Vec<AbstractBlockEntry>,
) -> AbstractFunction {
    AbstractFunction {
        machine: machine(),
        attachment: None,
        nominal_cleanup_subject: None,
        entry: block_id(1),
        parameters: Vec::new(),
        structural_parameters,
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries,
        operations,
    }
}

fn target_plan(
    plan: &AbstractOperationPlan,
    blocks: Vec<TargetControlBlock>,
) -> TargetOperationPlan {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let function = &plan.functions[0];
    let shapes = function
        .structural_parameters
        .iter()
        .map(|parameter| {
            crate::structural_inputs::parameter_shape(parameter, &layouts).expect("parameter shape")
        })
        .collect();
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: shapes,
            result: None,
        },
    )
    .expect("call plan");
    let parameters = function
        .structural_parameters
        .iter()
        .zip(&call_plan.parameters)
        .map(|(parameter, placement)| TargetStructuralParameter {
            place: parameter.place,
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access: parameter.access,
            projected_qualifications: parameter.projected_qualifications.clone(),
            shape: placement.shape,
            placement: placement.clone(),
        })
        .collect();
    TargetOperationPlan {
        psi: plan.psi,
        target: HOST(),
        profile: None,
        entry: plan.entry,
        functions: vec![TargetFunction {
            machine: function.machine,
            attachment: None,
            nominal_cleanup_subject: None,
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: Vec::new(),
            },
            graph: TargetControlGraph {
                structural_types: plan.structural_types.clone(),
                call_plan,
                scalar_parameters: Vec::new(),
                parameters,
                entry: function.entry,
                blocks,
            },
        }],
        native_callback_arguments: Vec::new(),
        borrowed_storage_layouts: Vec::new(),
    }
}

fn abi_parameter(
    plan: &AbstractOperationPlan,
    parameter: &StructuralParameterDeclaration,
) -> TargetStructuralParameter {
    let layouts = crate::native_geometry::NativeGeometry::new(&plan.structural_types).unwrap();
    let shape =
        crate::structural_inputs::parameter_shape(parameter, &layouts).expect("parameter shape");
    let call_plan = calling_conventions::evaluate_call_plan(
        CallingPolicy::native_for_target(HOST()),
        &CallSignature {
            parameters: vec![shape],
            result: None,
        },
    )
    .expect("parameter call plan");
    let placement = call_plan
        .parameters
        .into_iter()
        .next()
        .expect("parameter placement");
    TargetStructuralParameter {
        place: parameter.place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        access: parameter.access,
        projected_qualifications: parameter.projected_qualifications.clone(),
        shape: placement.shape,
        placement,
    }
}

fn block(
    id: u64,
    operations: Vec<TargetUnitOperation>,
    terminator: TargetControlTerminator,
) -> TargetControlBlock {
    TargetControlBlock {
        block: block_id(id),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    }
}

fn return_unit(id: u64) -> TargetControlTerminator {
    TargetControlTerminator::Return {
        psi_edge: edge(id),
        cleanup_actions: Vec::new(),
    }
}

fn home(
    operation: OperationId,
    result: &StructuralOperationResult,
    layout: TargetStructuralHomeLayout,
) -> TargetStructuralHomeRequirement {
    TargetStructuralHomeRequirement {
        origin: TargetStructuralHomeOrigin::OperationResult {
            operation,
            result: result.clone(),
        },
        layout,
    }
}

fn entry(id: u64, operation_offset: usize) -> AbstractBlockEntry {
    AbstractBlockEntry {
        block: block_id(id),
        structural_parameters: Vec::new(),
        parameters: Vec::new(),
        operation_offset,
    }
}

fn parameter(structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    parameter_at(1, structural_type)
}

fn parameter_at(id: u64, structural_type: StructuralTypeId) -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: place(id),
        position: 0,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    }
}

fn structural_result(
    place: PlaceId,
    structural_type: StructuralTypeId,
) -> StructuralOperationResult {
    StructuralOperationResult {
        place,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    }
}

fn sum_type(
    id: u64,
    identity: TerminalIdentityIndex,
    names: &mut TerminalIdentityTableBuilder,
    payload: &[StructuralFieldDeclaration],
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: ty(id),
        identity,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        shape: StructuralTypeShape::Sum {
            cases: vec![
                StructuralCaseDeclaration {
                    id: case_id(1),
                    identity: names.intern("Left"),
                    fields: payload.to_vec(),
                },
                StructuralCaseDeclaration {
                    id: case_id(2),
                    identity: names.intern("Right"),
                    fields: Vec::new(),
                },
            ],
        },
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

fn reference_type(
    id: u64,
    identity: TerminalIdentityIndex,
    referent: StructuralTypeId,
) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        quotient_carrier: None,
        id: ty(id),
        identity,
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        shape: StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        },
        scalar_restriction: None,
        requires: Vec::new(),
    }
}

fn scalar_field(
    id: u64,
    identity: TerminalIdentityIndex,
    integer: IntegerType,
) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        inline_carrier: None,
        id: StructuralFieldId::new(id).unwrap(),
        identity,
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
        storage: None,
    }
}

fn u32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

fn u64_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn psi() -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([9; 32]),
    }
}

fn machine() -> MachineId {
    MachineId::new(1).unwrap()
}

fn block_id(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}

fn place(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}

fn op(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}

fn edge(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}

fn val(value: u64) -> ValueId {
    ValueId::new(value).unwrap()
}

fn ty(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn case_id(value: u64) -> StructuralCaseId {
    StructuralCaseId::new(value).unwrap()
}

fn boolean_result() -> AbstractResult {
    AbstractResult {
        value: val(1),
        scalar_type: ScalarType::Boolean,
    }
}
