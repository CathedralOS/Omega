use super::*;

#[test]
fn unit_call_target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()));
        }),
        StraightLineUnitCallReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(55_120).unwrap()],
            edges: vec![caller_return_edge()],
        },
        TerminalPsiProvenance {
            operations: vec![call_operation()],
            edges: vec![EdgeId::new(55_121).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![call_operation(), OperationId::new(55_122).unwrap()],
            edges: vec![caller_return_edge()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineUnitCallReturnTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: caller_return_edge(),
                source_value: ValueId::new(55_123).unwrap(),
                value: false,
            };
        }),
        StraightLineUnitCallReturnTranslationError::TargetOperation
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
        StraightLineUnitCallReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            let argument = target_structural_argument();
            body.parameters.push(TargetStructuralParameter {
                place: argument.place,
                structural_type: argument.structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                projected_qualifications: Vec::new(),
                shape: argument.shape,
                placement: argument.source,
            });
        }),
        StraightLineUnitCallReturnTranslationError::TargetParameters
    );
}

#[test]
fn unit_call_and_return_field_corruption_fails_closed() {
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Call { psi_operation, .. } = operation else {
                unreachable!()
            };
            *psi_operation = OperationId::new(55_130).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Call { callee, .. } = operation else {
                unreachable!()
            };
            *callee = MachineId::new(55_131).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Call { arguments, .. } = operation else {
                unreachable!()
            };
            arguments.push(target_structural_argument());
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Call {
                claim_transfers, ..
            } = operation
            else {
                unreachable!()
            };
            claim_transfers.push(ClaimTransfer {
                claim: ClaimId::new(55_132).unwrap(),
                argument_index: 0,
            });
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Call {
                requirement_obligations,
                ..
            } = operation
            else {
                unreachable!()
            };
            requirement_obligations.clear();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::Call {
                crash_continuations,
                ..
            } = operation
            else {
                unreachable!()
            };
            crash_continuations.clear();
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations[0]);
            }),
            StraightLineUnitCallReturnTranslationError::TargetCall
        );
    }
    for operation in [
        TargetUnitOperation::Return {
            psi_edge: EdgeId::new(55_133).unwrap(),
            cleanup_actions: Vec::new(),
        },
        TargetUnitOperation::Return {
            psi_edge: caller_return_edge(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(55_134).unwrap(),
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
            StraightLineUnitCallReturnTranslationError::TargetReturn
        );
    }
    for mutate in [
        |operations: &mut Vec<TargetUnitOperation>| operations.clear(),
        |operations: &mut Vec<TargetUnitOperation>| operations.swap(0, 1),
        |operations: &mut Vec<TargetUnitOperation>| {
            operations.push(TargetUnitOperation::Return {
                psi_edge: caller_return_edge(),
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
            StraightLineUnitCallReturnTranslationError::TargetOperationRoster
        );
    }
}
