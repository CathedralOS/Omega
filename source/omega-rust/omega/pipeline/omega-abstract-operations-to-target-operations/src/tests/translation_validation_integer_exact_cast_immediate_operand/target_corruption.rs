use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.swap(0, 1)),
        StraightLineIntegerExactCastImmediateOperandTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.pop();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerExactCastImmediateOperandTranslationError::TargetProvenance
    );
}

#[test]
fn target_outer_cast_and_immediate_corruption_fail_closed() {
    for mutate in [
        mutate_edge as fn(&mut TargetOperation),
        mutate_outer_source_value,
        mutate_outer_type,
        mutate_cast_operation,
        mutate_obligation,
        mutate_source_type,
        mutate_immediate_source_value,
        mutate_immediate_value,
        substitute_operand,
        substitute_operation,
    ] {
        assert_eq!(
            candidate_error(|target| mutate(&mut target.functions[0].operation)),
            StraightLineIntegerExactCastImmediateOperandTranslationError::TargetOperation
        );
    }
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}

fn mutate_edge(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
        unreachable!()
    };
    *psi_edge = EdgeId::new(66_200).unwrap();
}

fn mutate_outer_source_value(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
        unreachable!()
    };
    *source_value = ValueId::new(66_201).unwrap();
}

fn mutate_outer_type(operation: &mut TargetOperation) {
    let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
        unreachable!()
    };
    *scalar_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
}

fn mutate_cast_operation(operation: &mut TargetOperation) {
    let TargetIntegerExpression::IntegerExactCast { psi_operation, .. } = expression(operation)
    else {
        unreachable!()
    };
    *psi_operation = OperationId::new(66_202).unwrap();
}

fn mutate_obligation(operation: &mut TargetOperation) {
    let TargetIntegerExpression::IntegerExactCast { obligation, .. } = expression(operation) else {
        unreachable!()
    };
    *obligation = ObligationId::new(66_203).unwrap();
}

fn mutate_source_type(operation: &mut TargetOperation) {
    let TargetIntegerExpression::IntegerExactCast { source_type, .. } = expression(operation)
    else {
        unreachable!()
    };
    *source_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
}

fn mutate_immediate_source_value(operation: &mut TargetOperation) {
    let TargetIntegerExpression::IntegerExactCast { operand, .. } = expression(operation) else {
        unreachable!()
    };
    let TargetIntegerExpression::Immediate { source_value, .. } = operand.as_mut() else {
        unreachable!()
    };
    *source_value = ValueId::new(66_204).unwrap();
}

fn mutate_immediate_value(operation: &mut TargetOperation) {
    let TargetIntegerExpression::IntegerExactCast { operand, .. } = expression(operation) else {
        unreachable!()
    };
    let TargetIntegerExpression::Immediate { value, .. } = operand.as_mut() else {
        unreachable!()
    };
    *value = IntegerValue::Unsigned(7);
}

fn substitute_operand(operation: &mut TargetOperation) {
    let TargetIntegerExpression::IntegerExactCast { operand, .. } = expression(operation) else {
        unreachable!()
    };
    *operand = Box::new(TargetIntegerExpression::Parameter {
        source_value: ValueId::new(66_004).unwrap(),
        parameter_index: 0,
        location: ScalarParameterLocation::IncomingStack { byte_offset: 0 },
    });
}

fn substitute_operation(operation: &mut TargetOperation) {
    *operation = TargetOperation::ReturnIntegerImmediate {
        psi_edge: EdgeId::new(66_007).unwrap(),
        source_value: ValueId::new(66_006).unwrap(),
        scalar_type: target_type(),
        value: IntegerValue::Unsigned(255),
    };
}
