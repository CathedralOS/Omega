use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineSaturatingIntegerAddImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineSaturatingIntegerAddImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineSaturatingIntegerAddImmediateTranslationError::TargetProvenance
    );
}

#[test]
fn every_target_immediate_axis_fails_closed() {
    for mutate in [
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(80_111).unwrap();
        },
        |operation: &mut TargetOperation| {
            let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
                unreachable!()
            };
            *source_value = ValueId::new(80_112).unwrap();
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
                psi_edge: EdgeId::new(80_009).unwrap(),
                source_value: ValueId::new(80_008).unwrap(),
                scalar_type: scalar_type(),
                expression: TargetIntegerExpression::SaturatingAdd {
                    psi_operation: OperationId::new(80_007).unwrap(),
                    left: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(80_004).unwrap(),
                        value: IntegerValue::Unsigned(65_530),
                    }),
                    right: Box::new(TargetIntegerExpression::Immediate {
                        source_value: ValueId::new(80_006).unwrap(),
                        value: IntegerValue::Unsigned(15),
                    }),
                },
            };
        },
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineSaturatingIntegerAddImmediateTranslationError::TargetOperation
        );
    }
}
