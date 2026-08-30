use super::*;

#[test]
fn candidate_operation_and_provenance_corruption_fails_closed() {
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(800).unwrap()],
            edges: vec![EdgeId::new(106).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![
                OperationId::new(103).unwrap(),
                OperationId::new(801).unwrap(),
            ],
            edges: vec![EdgeId::new(106).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(103).unwrap()],
            edges: vec![EdgeId::new(802).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(103).unwrap()],
            edges: vec![EdgeId::new(106).unwrap(), EdgeId::new(803).unwrap()],
        },
    ] {
        assert!(matches!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            AbstractToTargetTranslationValidationError::FunctionFamily {
                family: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
                error: AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate(
                    StraightLineIntegerImmediateTranslationError::TargetProvenance
                ),
                ..
            }
        ));
    }
    assert!(matches!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnBooleanImmediate {
                psi_edge: EdgeId::new(106).unwrap(),
                source_value: ValueId::new(104).unwrap(),
                value: true,
            };
        }),
        AbstractToTargetTranslationValidationError::FunctionFamily {
            family: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
            error: AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate(
                StraightLineIntegerImmediateTranslationError::TargetOperation
            ),
            ..
        }
    ));
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(804).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(805).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { scalar_type, .. } = operation else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Signed, 64);
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { value, .. } = operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(38);
        },
    ] {
        assert!(matches!(
            candidate_error(|candidate| mutate(&mut candidate.functions[0].operation)),
            AbstractToTargetTranslationValidationError::FunctionFamily {
                family: AbstractToTargetTranslationFamily::StraightLineIntegerImmediate,
                error: AbstractToTargetTranslationFamilyError::StraightLineIntegerImmediate(
                    StraightLineIntegerImmediateTranslationError::TargetOperation
                ),
                ..
            }
        ));
    }
}
