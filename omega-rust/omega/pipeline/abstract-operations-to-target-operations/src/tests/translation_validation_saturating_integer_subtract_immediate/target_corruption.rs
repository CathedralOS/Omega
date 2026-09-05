use super::*;
use target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineSaturatingIntegerSubtractImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineSaturatingIntegerSubtractImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineSaturatingIntegerSubtractImmediateTranslationError::TargetProvenance
    );
}

#[test]
fn every_target_immediate_axis_fails_closed() {
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(81_111).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(81_112).unwrap();
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
            *value = IntegerValue::Unsigned(1);
        },
        |operation: &mut TargetOperation| {
            *operation = TargetOperation::ReturnIntegerExpression {
                psi_edge: EdgeId::new(81_009).unwrap(),
                source_value: ValueId::new(81_008).unwrap(),
                scalar_type: scalar_type(),
                expression: TargetIntegerExpression::SaturatingSubtract {
                    psi_operation: OperationId::new(81_007).unwrap(),
                    left: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(81_004).unwrap(),
                        value: IntegerValue::Unsigned(5),
                    }),
                    right: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(81_006).unwrap(),
                        value: IntegerValue::Unsigned(10),
                    }),
                },
            };
        },
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineSaturatingIntegerSubtractImmediateTranslationError::TargetOperation
        );
    }
}
