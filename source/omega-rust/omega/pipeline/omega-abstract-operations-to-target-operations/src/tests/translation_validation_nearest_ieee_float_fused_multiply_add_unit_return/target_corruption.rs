use super::*;

fn operations(
    candidate: &mut omega_target_operations::TargetOperationPlan,
) -> &mut Vec<TargetUnitOperation> {
    let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
        unreachable!()
    };
    &mut body.operations
}

#[test]
fn target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| candidate.functions[0].fixed_integer_scalar_abi = Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()))),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    assert_eq!(
        candidate_error(
            |candidate| candidate.functions[0].provenance = TerminalPsiProvenance::default()
        ),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|candidate| candidate.functions[0].operation =
            TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(61_130).unwrap(),
                value: false,
            }),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetOperation
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
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.parameters.push(target_structural_parameter());
        }),
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetParameters
    );
}

#[test]
fn target_operation_roster_constants_and_fma_corruption_fails_closed() {
    for mutate in [
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.remove(0);
        },
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.swap(3, 4);
        },
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.push(TargetUnitOperation::Return {
                psi_edge: EdgeId::new(61_131).unwrap(),
                cleanup_actions: Vec::new(),
            });
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(operations(candidate))),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetOperationRoster
        );
    }
    for position in 0..3 {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetUnitOperation::IeeeFloatConstant { value, .. } =
                    &mut operations(candidate)[position]
                else {
                    unreachable!()
                };
                *value = IeeeFloatValue::Binary32(0x7fc0_0100 + position as u32);
            }),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetConstant
        );
    }
    for field in 0..6 {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd { psi_operation, result, format, left, right, addend, .. } = &mut operations(candidate)[3] else { unreachable!() };
                match field {
                    0 => *psi_operation = OperationId::new(61_140).unwrap(),
                    1 => *result = ValueId::new(61_141).unwrap(),
                    2 => *format = IeeeFloatFormat::Binary64,
                    3 => left.source_value = ValueId::new(61_142).unwrap(),
                    4 => right.defining_operation = OperationId::new(61_143).unwrap(),
                    _ => addend.value = IeeeFloatValue::Binary32(0x8000_0001),
                }
            }),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetFusedMultiplyAdd
        );
    }
}

#[test]
fn target_settlement_and_return_corruption_fails_closed() {
    for field in 0..6 {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd { settlement, .. } = &mut operations(candidate)[3] else { unreachable!() };
                match field {
                    0 => settlement.terminal_operation = OperationId::new(61_150).unwrap(),
                    1 => settlement.provider_plan_report_identity ^= 1,
                    2 => settlement.provider_plan_digest[0] ^= 1,
                    3 => settlement.format = IeeeFloatFormat::Binary64,
                    4 => settlement.slot = X86ScalarFmaSlot::Binary64,
                    _ => settlement.provider = omega_target::AdmittedX86ScalarFmaProvider::from_deployment_claim(TargetProfile::WindowsX64, &X86_SCALAR_FMA_REQUIRED_FEATURES).unwrap(),
                }
            }),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetSettlement
        );
    }
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Return { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(61_151).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Return {
                cleanup_actions, ..
            } = operation
            else {
                unreachable!()
            };
            cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(61_152).unwrap(),
            ));
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(&mut operations(candidate)[4])),
            StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError::TargetReturn
        );
    }
}

#[test]
fn whole_plan_structural_type_roster_corruption_fails_closed() {
    let target = NativeTarget::linux_x64();
    let format = IeeeFloatFormat::Binary32;
    let source = base_plan(format);
    let plan = provider_plan(target, format);
    let admitted = settlement(target, format, &plan);
    let mut candidate = lowered(&source, target, &[admitted]);
    let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
        unreachable!()
    };
    body.structural_types[0].identity = "test::substituted_fma_type".into();
    assert_eq!(
        validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
            &source,
            target,
            &candidate,
            &[admitted]
        ),
        Err(
            AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                machine: machine()
            }
        )
    );
}
