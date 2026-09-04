use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineWrappingIntegerAddImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineWrappingIntegerAddImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineWrappingIntegerAddImmediateTranslationError::TargetProvenance
    );
}

#[test]
fn target_immediate_corruption_fails_closed() {
    for mutate in [
        mutate_edge as fn(&mut TargetOperation),
        mutate_source_value,
        mutate_scalar_type,
        mutate_value,
        substitute_operation,
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineWrappingIntegerAddImmediateTranslationError::TargetOperation
        );
    }
}

fn mutate_edge(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { psi_edge, .. } = operation else {
        unreachable!()
    };
    *psi_edge = EdgeId::new(73_200).unwrap();
}

fn mutate_source_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { source_value, .. } = operation else {
        unreachable!()
    };
    *source_value = ValueId::new(73_201).unwrap();
}

fn mutate_scalar_type(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { scalar_type, .. } = operation else {
        unreachable!()
    };
    *scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
}

fn mutate_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerImmediate { value, .. } = operation else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(0);
}

fn substitute_operation(operation: &mut TargetOperation) {
    *operation = TargetOperation::ReturnIntegerExpression {
        psi_edge: EdgeId::new(73_009).unwrap(),
        source_value: ValueId::new(73_008).unwrap(),
        scalar_type: scalar_type(),
        expression: TargetIntegerExpression::WrappingAdd {
            psi_operation: OperationId::new(73_007).unwrap(),
            left: Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(73_004).unwrap(),
                value: IntegerValue::Unsigned(0x55),
            }),
            right: Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(73_006).unwrap(),
                value: IntegerValue::Unsigned(0x0f),
            }),
        },
    };
}
