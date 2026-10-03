//! Read-byte legalization retains exact result, target home and cleanup custody.
use crate::legalize_target_operations;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId,
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use target::NativeTarget;
use target_operations::{CompilerBuiltinExecution, TargetUnitOperation};
use terminal_psi::{StructuralMultiplicity, TerminalAffineCleanupAction};

pub(crate) fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    source
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::ByteRead".into(),
            shape: terminal_psi::StructuralTypeShape::Sum {
                cases: vec![
                    terminal_psi::StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "test::Empty".into(),
                        fields: vec![],
                    },
                    terminal_psi::StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: "test::Present".into(),
                        fields: vec![terminal_psi::StructuralFieldDeclaration {
                            id: StructuralFieldId::new(1).unwrap(),
                            identity: "test::payload".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: terminal_psi::StructuralFieldType::Scalar(
                                ScalarType::Integer(
                                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                                ),
                            ),
                        }],
                    },
                ],
            },
        });
    source
        .boundary_machines
        .push(terminal_psi::BoundaryMachineDeclaration {
            unconditional_termination: false,
            completion: terminal_psi::BoundaryCompletion::NormalReturn,
            kind: terminal_psi::BoundaryRequirementKind::Trait,
            scalar_ensures: Vec::new(),
            scalar_requires: Vec::new(),
            fixed_service_reach: Vec::new(),
            id: boundary,
            identity: "test::input".into(),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: vec![],
            structural_parameters: vec![],
            result: terminal_psi::BoundaryMachineResult::Structural(
                terminal_psi::BoundaryStructuralResultDeclaration {
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: vec![],
                },
            ),
            requires: vec![],
            program_local_root_introductions: vec![],
            content_guarantees: vec![],
            published_service_ceiling: vec![],
            crash_routes: Vec::new(),
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(1).unwrap(),
            boundary,
            result: AbstractBoundaryResult::Structural(terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                claims: vec![],
            }),
            arguments: vec![],
            structural_arguments: vec![],
            completion_claim_sources: vec![],
            completion_receipts: vec![],
        },
    );
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut source.functions[0].operations[1]
    else {
        unreachable!()
    };
    cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &[AdmittedBoundarySettlement {
                boundary,
                execution: AdmittedBoundaryExecution::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedReadByte,
                ),
                realization: target_operations::HostedReadByteRealization.into(),
            }],
            installation: None,
            native_callbacks: &[],
        },
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

#[test]
fn read_byte_target_replay_rejects_home_and_builtin_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64());
    for mutation in 0..8 {
        let mut changed = target.clone();
        let body = &mut changed.functions[0].graph;
        let TargetUnitOperation::BoundarySettlement {
            execution,
            realization,
            result,
            ..
        } = &mut body.blocks[0].operations[0]
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                *execution = target_operations::BoundaryExecutionBinding::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedWriteByteI32,
                )
            }
            1 => {
                *realization =
                    target_operations::BoundaryRealization::HostedWriteByteI32(Default::default())
            }
            2 => *result = target_operations::TargetBoundaryResult::Unit,
            3 | 4 => {
                let target_operations::TargetBoundaryResult::Structural(home) = result else {
                    unreachable!()
                };
                let target_operations::TargetStructuralHomeOrigin::OperationResult {
                    operation,
                    result,
                } = &mut home.origin
                else {
                    panic!("operation");
                };
                if mutation == 3 {
                    *operation = OperationId::new(2).unwrap();
                } else {
                    result.place = PlaceId::new(2).unwrap();
                }
            }
            5 => changed.target = NativeTarget::macos_arm64(),
            6 => {
                let target_operations::TargetControlTerminator::Return {
                    cleanup_actions, ..
                } = &mut body.blocks[0].terminator
                else {
                    unreachable!()
                };
                cleanup_actions.clear();
            }
            7 => {
                let target_operations::TargetBoundaryResult::Structural(home) = result else {
                    unreachable!()
                };
                let target_operations::TargetStructuralHomeLayout::Sum(layout) = &mut home.layout
                else {
                    unreachable!()
                };
                layout.cases[1].fields[0].byte_offset = 0;
            }
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn read_byte_rejects_same_width_unsigned_payload() {
    let native = NativeTarget::linux_x64();
    let (mut source, _, _) = fixture(native);
    let terminal_psi::StructuralTypeShape::Sum { cases } =
        &mut source.structural_types.make_mut()[0].shape
    else {
        unreachable!()
    };
    cases[1].fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    ));
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &[AdmittedBoundarySettlement {
                boundary: BoundaryMachineId::new(1).unwrap(),
                execution: AdmittedBoundaryExecution::CompilerBuiltin(
                    CompilerBuiltinExecution::HostedReadByte,
                ),
                realization: target_operations::HostedReadByteRealization.into(),
            }],
            installation: None,
            native_callbacks: &[],
        },
    );
    assert!(
        target.is_err(),
        "same-width unsigned payload does not satisfy the realization contract"
    );
}
