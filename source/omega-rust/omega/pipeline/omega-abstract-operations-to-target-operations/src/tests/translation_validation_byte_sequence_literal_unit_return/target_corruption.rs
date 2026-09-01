use super::*;

#[test]
fn byte_sequence_literal_target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()));
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(57_120).unwrap()],
            edges: vec![return_edge()],
        },
        TerminalPsiProvenance {
            operations: vec![establishment_operation()],
            edges: vec![EdgeId::new(57_121).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![establishment_operation(), OperationId::new(57_122).unwrap()],
            edges: vec![return_edge()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(57_123).unwrap(),
                value: false,
            };
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetOperation
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
        StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            body.parameters.push(target_structural_parameter());
        }),
        StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetParameters
    );
}

#[test]
fn byte_sequence_literal_target_semantic_corruption_fails_closed() {
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::EstablishByteSequenceLiteral { psi_operation, .. } = operation
            else {
                unreachable!()
            };
            *psi_operation = OperationId::new(57_130).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::EstablishByteSequenceLiteral { place, .. } = operation else {
                unreachable!()
            };
            place.id = PlaceId::new(57_131).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::EstablishByteSequenceLiteral {
                structural_type, ..
            } = operation
            else {
                unreachable!()
            };
            structural_type.identity = "Substituted".into();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::EstablishByteSequenceLiteral { bytes, .. } = operation else {
                unreachable!()
            };
            bytes.reverse();
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations[0]);
            }),
            StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetEstablishment
        );
    }
    for operation in [
        TargetUnitOperation::Return {
            psi_edge: EdgeId::new(57_132).unwrap(),
            cleanup_actions: Vec::new(),
        },
        TargetUnitOperation::Return {
            psi_edge: return_edge(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place_id())],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                body.operations[1] = operation;
            }),
            StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetReturn
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
            StraightLineByteSequenceLiteralUnitReturnTranslationError::TargetOperationRoster
        );
    }
}
