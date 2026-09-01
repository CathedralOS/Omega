use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineIntegerEqualImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineIntegerEqualImmediateTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerEqualImmediateTranslationError::TargetProvenance
    );
}

#[test]
fn target_immediate_corruption_fails_closed() {
    for mutate in [
        mutate_edge as fn(&mut TargetOperation),
        mutate_source_value,
        mutate_value,
        substitute_operation,
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineIntegerEqualImmediateTranslationError::TargetOperation
        );
    }
}

fn mutate_edge(operation: &mut TargetOperation) {
    let TargetOperation::ReturnBooleanImmediate { psi_edge, .. } = operation else {
        unreachable!()
    };
    *psi_edge = EdgeId::new(70_200).unwrap();
}

fn mutate_source_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnBooleanImmediate { source_value, .. } = operation else {
        unreachable!()
    };
    *source_value = ValueId::new(70_201).unwrap();
}

fn mutate_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnBooleanImmediate { value, .. } = operation else {
        unreachable!()
    };
    *value = true;
}

fn substitute_operation(operation: &mut TargetOperation) {
    *operation = TargetOperation::ReturnBooleanExpression {
        psi_edge: EdgeId::new(70_009).unwrap(),
        source_value: ValueId::new(70_008).unwrap(),
        expression: TargetBooleanExpression::IntegerEqual {
            psi_operation: OperationId::new(70_007).unwrap(),
            scalar_type: scalar_type(),
            left: Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(70_004).unwrap(),
                value: IntegerValue::Unsigned(255),
            }),
            right: Box::new(TargetIntegerExpression::Immediate {
                source_value: ValueId::new(70_006).unwrap(),
                value: IntegerValue::Unsigned(256),
            }),
        },
    };
}
