//! Read-byte legalization retains exact result, target home and cleanup custody.
use crate::{legalize_target_operations, validate_legalized_operations};
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use legalized_operations::LegalizedScalarInstructionKind;
use semantic_vocabulary::{
    BoundaryMachineId, FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId,
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use target::NativeTarget;
use target_operations::{CompilerBuiltinExecution, TargetOperation, TargetUnitOperation};
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
            id: boundary,
            identity: "test::input".into(),
            attachment: None,
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
        });
    source.functions[0].operations.insert(
        0,
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(1).unwrap(),
            boundary,
            result: AbstractBoundaryResult::Structural(terminal_psi::StructuralOperationResult {
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
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &source, native, &[AdmittedBoundarySettlement {
            boundary, execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::LinuxReadByte),
            realization: target_operations::LinuxReadByteRealization.into(),
        }],
    ).unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

#[test]
fn read_byte_preserves_structural_result_without_scalar_definition() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let (source, target, unit) = fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        let function = &legal.plan().scalar_functions[0];
        let row = &function.blocks[0].instructions[0];
        assert!(row.result.is_none());
        assert!(row.has_valid_hosted_read_byte_shape());
        assert_eq!(
            function.structural.as_ref().unwrap().structural_places,
            unit.functions[0].structural_places
        );
        let legalized_operations::LegalizedScalarTerminator::Return(returned) =
            &function.blocks[0].terminator
        else {
            panic!("Unit return")
        };
        assert_eq!(
            returned.ownership,
            [optimization_unit::OwnershipEvent::Cleanup(vec![
                TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(1).unwrap())
            ])]
        );
    }
}

pub(crate) fn two_results_fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = fixture(native);
    let mut second = source.functions[0].operations[0].clone();
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result: AbstractBoundaryResult::Structural(result),
        ..
    } = &mut second
    else {
        unreachable!()
    };
    *psi_operation = OperationId::new(2).unwrap();
    result.place = PlaceId::new(2).unwrap();
    source.functions[0].operations.insert(1, second);
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut source.functions[0].operations[2]
    else {
        unreachable!()
    };
    cleanup_actions.insert(
        0,
        TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(2).unwrap()),
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &source, native, &[AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::LinuxReadByte),
            realization: target_operations::LinuxReadByteRealization.into(),
        }],
    ).unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

#[test]
fn read_byte_replay_rejects_result_layout_and_custody_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64());
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    let identity = legalized_operations::legalized_operation_plan_identity(legal.plan());
    for mutation in 0..12 {
        let mut changed = legal.plan().clone();
        let row = &mut changed.scalar_functions[0].blocks[0].instructions[0];
        let LegalizedScalarInstructionKind::HostedReadByte {
            boundary,
            result,
            layout,
        } = &mut row.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *boundary = BoundaryMachineId::new(2).unwrap(),
            1 => result.place = PlaceId::new(2).unwrap(),
            2 => result.structural_type = StructuralTypeId::new(2).unwrap(),
            3 => result.multiplicity = StructuralMultiplicity::Linear,
            4 => layout.payload_byte_offset = 0,
            5 => layout.cases.swap(0, 1),
            6 => row.fuel.clear(),
            7 => row.ownership.clear(),
            8 => row.operation = OperationId::new(2).unwrap(),
            9 => {
                let legalized_operations::LegalizedScalarTerminator::Return(returned) =
                    &mut changed.scalar_functions[0].blocks[0].terminator
                else {
                    unreachable!()
                };
                returned.ownership.clear();
            }
            10 => row.effect.output += 1,
            11 => {
                row.result = Some(legalized_operations::LegalizedValueDefinition {
                    value: semantic_vocabulary::ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    ),
                    definition_site: optimization_unit::ValueDefinitionSite::Node {
                        block: semantic_vocabulary::BlockId::new(1).unwrap(),
                        node: 0,
                    },
                })
            }
            _ => unreachable!(),
        }
        assert_ne!(
            legalized_operations::legalized_operation_plan_identity(&changed),
            identity
        );
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn read_byte_target_replay_rejects_home_and_builtin_substitution() {
    let (source, target, unit) = fixture(NativeTarget::linux_x64());
    for mutation in 0..8 {
        let mut changed = target.clone();
        let TargetOperation::UnitBody(body) = &mut changed.functions[0].operation else {
            unreachable!()
        };
        let TargetUnitOperation::BoundarySettlement {
            execution,
            realization,
            result,
            ..
        } = &mut body.operations[0]
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
                if mutation == 3 {
                    home.defining_operation = OperationId::new(2).unwrap();
                } else {
                    home.result.place = PlaceId::new(2).unwrap();
                }
            }
            5 => changed.target = NativeTarget::macos_arm64(),
            6 => {
                let TargetUnitOperation::Return {
                    cleanup_actions, ..
                } = &mut body.operations[1]
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
    let terminal_psi::StructuralTypeShape::Sum { cases } = &mut source.structural_types[0].shape
    else {
        unreachable!()
    };
    cases[1].fields[0].field_type = terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
    ));
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &source, native, &[AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(1).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(CompilerBuiltinExecution::LinuxReadByte),
            realization: target_operations::LinuxReadByteRealization.into(),
        }],
    ).expect("target sum layout alone does not distinguish signed payload meaning");
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    assert!(legalize_target_operations(&target, &source, &unit).is_err());
}
