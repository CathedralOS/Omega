use super::*;

#[test]
fn port_write_unit_return_target_envelope_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].fixed_integer_scalar_abi =
                Some(fixed_integer_scalar_abi(NativeTarget::linux_x64()));
        }),
        StraightLinePortWriteUnitReturnTranslationError::TargetFixedIntegerScalarAbi
    );
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(54_120).unwrap()],
            edges: vec![return_edge()],
        },
        TerminalPsiProvenance {
            operations: vec![port_operation()],
            edges: vec![EdgeId::new(54_121).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![port_operation(), OperationId::new(54_122).unwrap()],
            edges: vec![return_edge()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLinePortWriteUnitReturnTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: return_edge(),
                source_value: ValueId::new(54_123).unwrap(),
                value: false,
            };
        }),
        StraightLinePortWriteUnitReturnTranslationError::TargetOperation
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
        StraightLinePortWriteUnitReturnTranslationError::TargetCallPlan
    );
    assert_eq!(
        candidate_error(|candidate| {
            let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                unreachable!()
            };
            let shape = ValueShape::integer(4, 4);
            let placement = evaluate_call_plan(
                CallingPolicy::native_for_target(NativeTarget::linux_x64()),
                &CallSignature {
                    parameters: vec![shape],
                    result: None,
                },
            )
            .unwrap()
            .parameters
            .remove(0);
            body.parameters.push(TargetStructuralParameter {
                place: PlaceId::new(54_124).unwrap(),
                structural_type: StructuralTypeId::new(54_125).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::Owned,
                shape,
                placement,
            });
        }),
        StraightLinePortWriteUnitReturnTranslationError::TargetParameters
    );
}

#[test]
fn port_write_and_return_field_corruption_fails_closed() {
    for mutate in [
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::PortWrite { psi_operation, .. } = operation else {
                unreachable!()
            };
            *psi_operation = OperationId::new(54_130).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::PortWrite { service, .. } = operation else {
                unreachable!()
            };
            *service = ServiceId::new(54_131).unwrap();
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::PortWrite { port, .. } = operation else {
                unreachable!()
            };
            *port = 0x02f8;
        },
        |operation: &mut TargetUnitOperation| {
            let TargetUnitOperation::PortWrite { value, .. } = operation else {
                unreachable!()
            };
            *value = 0x42;
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| {
                let TargetOperation::UnitBody(body) = &mut candidate.functions[0].operation else {
                    unreachable!()
                };
                mutate(&mut body.operations[0]);
            }),
            StraightLinePortWriteUnitReturnTranslationError::TargetPortWrite
        );
    }
    for operation in [
        TargetUnitOperation::Return {
            psi_edge: EdgeId::new(54_132).unwrap(),
            cleanup_actions: Vec::new(),
        },
        TargetUnitOperation::Return {
            psi_edge: return_edge(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(54_133).unwrap(),
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
            StraightLinePortWriteUnitReturnTranslationError::TargetReturn
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
            StraightLinePortWriteUnitReturnTranslationError::TargetOperationRoster
        );
    }
}
