//! Lawful owned read-result case graph shared by producer and receiving tests.
use abstract_operations::{
    AbstractBlockEntry, AbstractOperation, AbstractOperationPlan, AbstractParameter,
    AbstractStructuralCasePayloadBinding, AbstractStructuralCaseSuccessor,
};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType,
    OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, ValueId,
};
use target_operations::TargetOperationPlan;

pub(crate) fn fixture(
    native: ::target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut plan, _, _) = crate::tests::legalization::byte_input::fixture(native);
    let function = &mut plan.functions[0];
    let entry = function.entry;
    let empty = BlockId::new(20).unwrap();
    let present = BlockId::new(30).unwrap();
    let join = BlockId::new(40).unwrap();
    let parameter = ValueId::new(10).unwrap();
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let read = function.operations[0].clone();
    let jump = |edge| AbstractOperation::Jump {
        psi_edge: EdgeId::new(edge).unwrap(),
        target: join,
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    function.operations = vec![
        read,
        AbstractOperation::StructuralCase {
            source: PlaceId::new(1).unwrap(),
            cases: vec![
                AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(1).unwrap(),
                    target: empty,
                    case: StructuralCaseId::new(1).unwrap(),
                    payloads: Vec::new(),
                    trivial_affine_discards: vec![PlaceId::new(1).unwrap()],
                },
                AbstractStructuralCaseSuccessor {
                    psi_edge: EdgeId::new(2).unwrap(),
                    target: present,
                    case: StructuralCaseId::new(2).unwrap(),
                    payloads: vec![AbstractStructuralCasePayloadBinding {
                        parameter,
                        field: StructuralFieldId::new(1).unwrap(),
                        scalar_type,
                    }],
                    trivial_affine_discards: vec![PlaceId::new(1).unwrap()],
                },
            ],
        },
        jump(3),
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(5).unwrap(),
            cleanup_actions: Vec::new(),
        },
        jump(4),
    ];
    function.block_entries = [
        (entry, 0, false),
        (empty, 2, false),
        (join, 3, false),
        (present, 4, true),
    ]
    .into_iter()
    .map(|(block, operation_offset, payload)| AbstractBlockEntry {
        block,
        operation_offset,
        structural_parameters: Vec::new(),
        parameters: if payload {
            vec![AbstractParameter {
                value: parameter,
                scalar_type,
            }]
        } else {
            Vec::new()
        },
    })
    .collect();
    function.operations.insert(
        4,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(2).unwrap(),
            boundary: BoundaryMachineId::new(2).unwrap(),
            result: abstract_operations::AbstractBoundaryResult::Unit,
            arguments: vec![parameter],
            structural_arguments: Vec::new(),
            completion_claim_sources: Vec::new(),
            completion_receipts: Vec::new(),
        },
    );
    let mut output = plan.boundary_machines[0].clone();
    output.id = BoundaryMachineId::new(2).unwrap();
    output.identity = "test::output".into();
    output.parameter_order = vec![terminal_psi::BoundaryParameterKind::Scalar];
    output.scalar_parameters = vec![scalar_type];
    output.result = terminal_psi::BoundaryMachineResult::Unit;
    plan.boundary_machines.push(output);
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &[
                AdmittedBoundarySettlement {
                    boundary: BoundaryMachineId::new(1).unwrap(),
                    execution: AdmittedBoundaryExecution::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedReadByte,
                    ),
                    realization: target_operations::HostedReadByteRealization.into(),
                },
                AdmittedBoundarySettlement {
                    boundary: BoundaryMachineId::new(2).unwrap(),
                    execution: AdmittedBoundaryExecution::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                    ),
                    realization: target_operations::HostedWriteByteI32Realization.into(),
                },
            ],
            installation: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
        },
    )
    .expect("case arms can jump to an ordinary returning join in non-topological roster order");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("case ownership and payload graph");
    (plan, target, unit)
}

/// The same case graph dispatched on the function's own owned sum parameter:
/// no producer establishes a home, and each arm discards the arrival when it
/// jumps to the join.
pub(crate) fn parameter_fixture(
    native: ::target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    parameter_fixture_for(native, false)
}

/// The same owned-parameter dispatch over a mixed carrier: common fields sit
/// beside the dispatched alternatives in one conventional sum layout.
pub(crate) fn mixed_parameter_fixture(
    native: ::target::NativeTarget,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    parameter_fixture_for(native, true)
}

fn parameter_fixture_for(
    native: ::target::NativeTarget,
    mixed: bool,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut plan, _, _) = fixture(native);
    let place = PlaceId::new(1).unwrap();
    let function = &mut plan.functions[0];
    let AbstractOperation::BoundaryCall {
        result: abstract_operations::AbstractBoundaryResult::Structural(read),
        ..
    } = function.operations.remove(0)
    else {
        panic!("read producer");
    };
    for entry in &mut function.block_entries[1..] {
        entry.operation_offset -= 1;
    }
    let structural_type = if mixed {
        // A mixed carrier keeps the same dispatched cases and adds common
        // fields every alternative shares; dispatch still reads the tag.
        let terminal_psi::StructuralTypeShape::Sum { cases } =
            plan.structural_types.make_mut()[0].shape.clone()
        else {
            panic!("sum source shape")
        };
        let mixed_type = semantic_vocabulary::StructuralTypeId::new(7).unwrap();
        plan.structural_types
            .make_mut()
            .push(terminal_psi::StructuralTypeDeclaration {
                id: mixed_type,
                identity: "test::MixedCarrier".into(),
                shape: terminal_psi::StructuralTypeShape::Mixed {
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        id: StructuralFieldId::new(7).unwrap(),
                        identity: "test::common".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                        )),
                    }],
                    cases,
                },
            });
        mixed_type
    } else {
        read.structural_type
    };
    function
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: read.multiplicity,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    for operation in &mut function.operations {
        match operation {
            AbstractOperation::StructuralCase { cases, .. } => {
                for case in cases {
                    case.trivial_affine_discards.clear();
                }
            }
            AbstractOperation::Jump {
                trivial_affine_discards,
                ..
            } => *trivial_affine_discards = vec![place],
            _ => {}
        }
    }
    plan.boundary_machines.remove(0);
    let target = if mixed {
        mixed_parameter_target(&plan, native)
    } else {
        abstract_operations_to_target_operations::lower_to_target_operations(
            &plan,
            abstract_operations_to_target_operations::TargetLoweringRequest {
                target: native,
                settlements: &[AdmittedBoundarySettlement {
                    boundary: BoundaryMachineId::new(2).unwrap(),
                    execution: AdmittedBoundaryExecution::CompilerBuiltin(
                        target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                    ),
                    realization: target_operations::HostedWriteByteI32Realization.into(),
                }],
                installation: None,
                ieee_float_fma: &[],
                native_callbacks: &[],
            },
        )
        .expect("an owned sum parameter dispatches without a producer home")
    };
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("parameter case ownership and payload graph");
    (plan, target, unit)
}

/// The target plan an owned mixed-parameter dispatch lowers to: one tagged
/// parameter observed in place, the same arms and settlements the sum
/// dispatch carries.
fn mixed_parameter_target(
    plan: &AbstractOperationPlan,
    native: ::target::NativeTarget,
) -> TargetOperationPlan {
    let i32_shape = calling_conventions::ValueShape::integer(4, 4);
    let layout = calling_conventions::evaluate_conventional_sum_layout(
        &[i32_shape],
        &[Vec::new(), vec![i32_shape]],
    )
    .unwrap();
    let payload_offset = u32::from(layout.cases[1].fields[0].byte_offset);
    let policy = calling_conventions::CallingPolicy::native_for_target(native);
    let call_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![layout.shape],
            result: None,
        },
    )
    .unwrap();
    let boundary_plan = calling_conventions::evaluate_call_plan(
        policy,
        &calling_conventions::CallSignature {
            parameters: vec![i32_shape],
            result: None,
        },
    )
    .unwrap();
    let place = PlaceId::new(1).unwrap();
    let parameter = target_operations::TargetStructuralParameter {
        place,
        structural_type: semantic_vocabulary::StructuralTypeId::new(7).unwrap(),
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        access: terminal_psi::StructuralAccess::Owned,
        projected_qualifications: Vec::new(),
        shape: layout.shape,
        placement: call_plan.parameters[0].clone(),
    };
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let present_value = target_operations::TargetScalarBlockValue {
        block: BlockId::new(30).unwrap(),
        value: ValueId::new(10).unwrap(),
        scalar_type,
    };
    let discard = terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place);
    target_operations::TargetOperationPlan {
        psi: plan.psi.clone(),
        target: native,
        entry: semantic_vocabulary::MachineId::new(1).unwrap(),
        functions: vec![target_operations::TargetFunction {
            machine: semantic_vocabulary::MachineId::new(1).unwrap(),
            attachment: None,
            scalar_abi: None,
            mixed_structural_scalar_abi: None,
            provenance: target_operations::TerminalPsiProvenance {
                operations: vec![OperationId::new(2).unwrap()],
                edges: [1, 2, 3, 5, 4]
                    .iter()
                    .map(|edge| EdgeId::new(*edge).unwrap())
                    .collect(),
            },
            graph: target_operations::TargetControlGraph {
                structural_types: plan.structural_types.clone(),
                call_plan,
                scalar_parameters: Vec::new(),
                parameters: vec![parameter.clone()],
                dynamic_parameters: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![
                    target_operations::TargetControlBlock {
                        block: BlockId::new(1).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: target_operations::TargetControlTerminator::StructuralCase {
                            source: target_operations::TargetStructuralCaseSource::Parameter {
                                parameter,
                                layout: target_operations::TargetStructuralHomeLayout::Sum(
                                    layout,
                                ),
                            },
                            cases: vec![
                                target_operations::TargetControlCaseSuccessor {
                                    psi_edge: EdgeId::new(1).unwrap(),
                                    case: StructuralCaseId::new(1).unwrap(),
                                    case_tag: 0,
                                    target: BlockId::new(20).unwrap(),
                                    payloads: Vec::new(),
                                    trivial_affine_discards: Vec::new(),
                                },
                                target_operations::TargetControlCaseSuccessor {
                                    psi_edge: EdgeId::new(2).unwrap(),
                                    case: StructuralCaseId::new(2).unwrap(),
                                    case_tag: 1,
                                    target: BlockId::new(30).unwrap(),
                                    payloads: vec![
                                        target_operations::TargetControlCasePayload {
                                            field: StructuralFieldId::new(1).unwrap(),
                                            field_byte_offset: payload_offset,
                                            parameter: present_value,
                                        },
                                    ],
                                    trivial_affine_discards: Vec::new(),
                                },
                            ],
                        },
                    },
                    target_operations::TargetControlBlock {
                        block: BlockId::new(20).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: target_operations::TargetControlTerminator::Jump {
                            successor: target_operations::TargetControlSuccessor {
                                psi_edge: EdgeId::new(3).unwrap(),
                                target: BlockId::new(40).unwrap(),
                                bindings: Vec::new(),
                                structural_bindings: Vec::new(),
                                cleanup_actions: vec![discard.clone()],
                            },
                        },
                    },
                    target_operations::TargetControlBlock {
                        block: BlockId::new(40).unwrap(),
                        parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        operations: Vec::new(),
                        terminator: target_operations::TargetControlTerminator::Return {
                            psi_edge: EdgeId::new(5).unwrap(),
                            cleanup_actions: Vec::new(),
                        },
                    },
                    target_operations::TargetControlBlock {
                        block: BlockId::new(30).unwrap(),
                        parameters: vec![target_operations::TargetScalarBlockParameter {
                            value: ValueId::new(10).unwrap(),
                            scalar_type,
                        }],
                        structural_parameters: Vec::new(),
                        operations: vec![
                            target_operations::TargetUnitOperation::BoundarySettlement {
                                psi_operation: OperationId::new(2).unwrap(),
                                boundary: BoundaryMachineId::new(2).unwrap(),
                                result: target_operations::TargetBoundaryResult::Unit,
                                execution:
                                    target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                                        target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
                                    ),
                                realization:
                                    target_operations::BoundaryRealization::HostedWriteByteI32(
                                        target_operations::HostedWriteByteI32Realization,
                                    ),
                                scalar_arguments: Vec::new(),
                                runtime_scalar_arguments: vec![
                                    target_operations::TargetUnitScalarCallArgument {
                                        parameter_index: 0,
                                        source:
                                            target_operations::TargetUnitScalarArgumentSource::BlockParameter(
                                                present_value,
                                            ),
                                        placement: boundary_plan.parameters[0].clone(),
                                    },
                                ],
                                arguments: Vec::new(),
                                byte_sequence_arguments: Vec::new(),
                                completion_claim_sources: Vec::new(),
                                completion_receipts: Vec::new(),
                            },
                        ],
                        terminator: target_operations::TargetControlTerminator::Jump {
                            successor: target_operations::TargetControlSuccessor {
                                psi_edge: EdgeId::new(4).unwrap(),
                                target: BlockId::new(40).unwrap(),
                                bindings: Vec::new(),
                                structural_bindings: Vec::new(),
                                cleanup_actions: vec![discard],
                            },
                        },
                    },
                ],
            },
        }],
        native_callback_arguments: Vec::new(),
    }
}
