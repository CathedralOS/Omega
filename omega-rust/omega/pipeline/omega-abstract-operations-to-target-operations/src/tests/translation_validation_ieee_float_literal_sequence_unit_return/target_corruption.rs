use super::*;

#[test]
fn target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()));
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![
                OperationId::new(60_003).unwrap(),
                OperationId::new(60_005).unwrap(),
            ],
            edges: vec![return_edge()],
        },
        TerminalPsiProvenance {
            operations: vec![
                OperationId::new(60_005).unwrap(),
                OperationId::new(60_003).unwrap(),
                OperationId::new(60_007).unwrap(),
            ],
            edges: vec![return_edge()],
        },
        TerminalPsiProvenance {
            operations: LITERALS
                .iter()
                .map(|(operation, _, _)| OperationId::new(*operation).unwrap())
                .collect(),
            edges: vec![EdgeId::new(60_123).unwrap()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(60_124).unwrap(),
                value: false,
            };
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(NativeTarget::windows_x64()),
                &CallSignature::default(),
            )
            .unwrap();
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.parameters.push(target_structural_parameter());
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetParameters
    );
}

#[test]
fn target_literal_sequence_corruption_fails_closed() {
    for mutate in [
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.remove(1);
        },
        |operations: &mut Vec<TargetUnitOperation>| {
            operations[1] = TargetUnitOperation::IntegerConstant {
                psi_operation: OperationId::new(60_125).unwrap(),
                result: ValueId::new(60_126).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: psi_core::IntegerValue::Signed(1),
            };
        },
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.push(TargetUnitOperation::IeeeFloatConstant {
                psi_operation: OperationId::new(60_127).unwrap(),
                result: ValueId::new(60_128).unwrap(),
                value: IeeeFloatValue::Binary32(1),
            });
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations);
            }),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetOperationRoster
        );
    }

    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IeeeFloatConstant { psi_operation, .. } = operation else {
                unreachable!()
            };
            *psi_operation = OperationId::new(60_130).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IeeeFloatConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(60_131).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IeeeFloatConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary64(0xfff8_dead_beef_cafe);
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations[1]);
            }),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.operations.swap(0, 1);
        }),
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
    );
}

#[test]
fn target_return_corruption_fails_closed() {
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Return { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(60_140).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Return {
                cleanup_actions, ..
            } = operation
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(60_141).unwrap(),
            ));
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(body.operations.last_mut().unwrap());
            }),
            StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetReturn
        );
    }
}

#[test]
fn whole_plan_custody_rejects_sequence_structural_type_roster_corruption() {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();

    for mutate in [
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types.pop();
        },
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types.swap(0, 1);
        },
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types[0].identity = "test::substituted_sequence".into();
        },
        |body: &mut omega_target_operations::TargetUnitBody| {
            body.structural_types.push(StructuralTypeDeclaration {
                id: StructuralTypeId::new(60_150).unwrap(),
                identity: "test::injected_sequence".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            });
        },
    ] {
        let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
        let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
            unreachable!()
        };
        mutate(body);
        assert_eq!(
            validate_abstract_to_target_translation(&source, target_profile, &candidate),
            Err(
                AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                    machine: machine(),
                }
            )
        );
    }
}
