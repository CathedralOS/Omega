use super::*;

#[test]
fn ieee_literal_target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()));
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(59_120).unwrap()],
            edges: vec![return_edge()],
        },
        TerminalPsiProvenance {
            operations: vec![literal_operation()],
            edges: vec![EdgeId::new(59_121).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![literal_operation(), OperationId::new(59_122).unwrap()],
            edges: vec![return_edge()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(59_123).unwrap(),
                value: false,
            };
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetOperation
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
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.parameters.push(target_structural_parameter());
        }),
        StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetParameters
    );
}

#[test]
fn ieee_literal_target_semantic_corruption_fails_closed() {
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IeeeFloatConstant { psi_operation, .. } = operation else {
                unreachable!()
            };
            *psi_operation = OperationId::new(59_130).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IeeeFloatConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(59_131).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IeeeFloatConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary32(0x8000_0000);
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations[0]);
            }),
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetConstant
        );
    }
    for operation in [
        TargetUnitOperation::Return {
            psi_edge: EdgeId::new(59_132).unwrap(),
            cleanup_actions: Vec::new(),
        },
        TargetUnitOperation::Return {
            psi_edge: return_edge(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(59_133).unwrap(),
            )],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                body.operations[1] = operation;
            }),
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetReturn
        );
    }
    for mutate in [
        |operations: &mut Vec<TargetUnitOperation>| operations.clear(),
        |operations: &mut Vec<TargetUnitOperation>| operations.swap(0, 1),
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.push(TargetUnitOperation::Return {
                psi_edge: return_edge(),
                cleanup_actions: Vec::new(),
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
            StraightLineIeeeFloatLiteralUnitReturnTranslationError::TargetOperationRoster
        );
    }
}

#[test]
fn whole_plan_custody_rejects_ieee_literal_structural_type_roster_corruption() {
    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();

    for mutate in [
        |body: &mut target_operations::TargetUnitBody| {
            body.structural_types.pop();
        },
        |body: &mut target_operations::TargetUnitBody| {
            body.structural_types.swap(0, 1);
        },
        |body: &mut target_operations::TargetUnitBody| {
            body.structural_types[0].identity = "test::substituted".into();
        },
        |body: &mut target_operations::TargetUnitBody| {
            body.structural_types.push(StructuralTypeDeclaration {
                id: StructuralTypeId::new(59_140).unwrap(),
                identity: "test::injected".into(),
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
