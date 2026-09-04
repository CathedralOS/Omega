use super::*;

#[test]
fn boolean_candidate_and_provenance_corruption_fails_closed() {
    for provenance in [
        TerminalPsiProvenance::default(),
        TerminalPsiProvenance {
            operations: vec![OperationId::new(1_110).unwrap()],
            edges: vec![EdgeId::new(1_006).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![
                OperationId::new(1_003).unwrap(),
                OperationId::new(1_122).unwrap(),
            ],
            edges: vec![EdgeId::new(1_006).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(1_003).unwrap()],
            edges: vec![EdgeId::new(1_111).unwrap()],
        },
        TerminalPsiProvenance {
            operations: vec![OperationId::new(1_003).unwrap()],
            edges: vec![EdgeId::new(1_006).unwrap(), EdgeId::new(1_123).unwrap()],
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| candidate.functions[0].provenance = provenance),
            StraightLineBooleanImmediateTranslationError::TargetProvenance
        );
    }
    assert_eq!(
        candidate_error(|candidate| {
            candidate.functions[0].operation = TargetOperation::ReturnIntegerImmediate {
                psi_edge: EdgeId::new(1_006).unwrap(),
                source_value: ValueId::new(1_004).unwrap(),
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: psi_core::IntegerValue::Unsigned(1),
            };
        }),
        StraightLineBooleanImmediateTranslationError::TargetOperation
    );
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnBooleanImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(1_112).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnBooleanImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(1_113).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnBooleanImmediate { value, .. } = operation else {
                unreachable!()
            };
            *value = false;
        },
    ] {
        assert_eq!(
            candidate_error(|candidate| mutate(&mut candidate.functions[0].operation)),
            StraightLineBooleanImmediateTranslationError::TargetOperation
        );
    }
}
