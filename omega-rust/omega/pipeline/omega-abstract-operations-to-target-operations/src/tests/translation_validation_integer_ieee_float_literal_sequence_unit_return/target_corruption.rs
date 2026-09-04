use super::*;

#[test]
fn target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(
            |candidate| candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()))
        ),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    assert_eq!(
        candidate_error(
            |candidate| candidate.functions[0].provenance = TerminalPsiProvenance::default()
        ),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|candidate| candidate.functions[0].operation =
            TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(63_123).unwrap(),
                value: false,
            }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetOperation
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
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.parameters.push(target_structural_parameter());
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetParameters
    );
}

#[test]
fn target_roster_kind_order_and_payload_corruption_fails_closed() {
    for mutate in [
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.remove(1);
        },
        |operations: &mut Vec<TargetUnitOperation>| {
            operations[1] = TargetUnitOperation::Return {
                psi_edge: EdgeId::new(63_125).unwrap(),
                cleanup_actions: Vec::new(),
            };
        },
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.push(TargetUnitOperation::Return {
                psi_edge: EdgeId::new(63_126).unwrap(),
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
            StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetOperationRoster
        );
    }
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IntegerConstant { result, .. } = operation else {
                unreachable!()
            };
            *result = ValueId::new(63_130).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IntegerConstant { scalar_type, .. } = operation else {
                unreachable!()
            };
            *scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::IntegerConstant { value, .. } = operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(1);
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations[0]);
            }),
            StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            let TargetUnitOperation::IeeeFloatConstant { value, .. } = &mut body.operations[1]
            else {
                unreachable!()
            };
            *value = IeeeFloatValue::Binary32(0x8000_0000);
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.operations.swap(0, 1);
        }),
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetConstant
    );
}

#[test]
fn target_return_and_whole_structural_roster_corruption_fails_closed() {
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Return { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(63_140).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Return {
                cleanup_actions, ..
            } = operation
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(63_141).unwrap(),
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
            StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError::TargetReturn
        );
    }

    let source = base_plan();
    let target_profile = NativeTarget::linux_x64();
    let mut candidate = lower_to_target_operations(&source, target_profile).unwrap();
    let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
        unreachable!()
    };
    body.structural_types[0].identity = "test::substituted_mixed_sequence".into();
    assert_eq!(
        validate_abstract_to_target_translation(&source, target_profile, &candidate),
        Err(
            AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                machine: machine()
            }
        )
    );
}
