use super::*;
use target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineWrappingIntegerShiftRightImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineWrappingIntegerShiftRightImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineWrappingIntegerShiftRightImmediateTranslationError::TargetProvenance
    );
}
#[test]
fn every_target_immediate_axis_fails_closed() {
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(83_111).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(83_112).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { scalar_type, .. } = operation else {
                unreachable!()
            };
            *scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { value, .. } = operation else {
                unreachable!()
            };
            *value = IntegerValue::Unsigned(0);
        },
        |operation: &mut TargetOperation| {
            *operation = TargetOperation::ReturnIntegerExpression {
                psi_edge: EdgeId::new(83_009).unwrap(),
                source_value: ValueId::new(83_008).unwrap(),
                scalar_type: value_type(),
                expression: TargetIntegerExpression::WrappingShiftRight {
                    psi_operation: OperationId::new(83_007).unwrap(),
                    count_type: count_type(),
                    value: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(83_004).unwrap(),
                        value: IntegerValue::Unsigned(65_535),
                    }),
                    count: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(83_006).unwrap(),
                        value: IntegerValue::Unsigned(1),
                    }),
                },
            };
        },
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineWrappingIntegerShiftRightImmediateTranslationError::TargetOperation
        );
    }
}
