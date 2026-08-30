use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn exact_cast_target_provenance_and_obligation_corruption_fail_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineIntegerExactCastParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineIntegerExactCastParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance = TerminalPsiProvenance::default()),
        StraightLineIntegerExactCastParameterTranslationError::TargetProvenance
    );
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerExactCast { obligation, .. } = expression(operation)
        else {
            unreachable!()
        };
        *obligation = ObligationId::new(45_100).unwrap();
    });
}

#[test]
fn exact_cast_outer_and_nested_target_corruption_fail_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(45_101).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerExactCast { psi_operation, .. } = expression(operation)
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(45_102).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerExactCast { source_type, .. } = expression(operation)
        else {
            unreachable!()
        };
        *source_type = integer_type(IntegerSign::Signed, 64);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerExactCast { operand, .. } = expression(operation)
        else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter {
            parameter_index, ..
        } = operand.as_mut()
        else {
            unreachable!()
        };
        *parameter_index = 1;
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerExactCast { operand, .. } = expression(operation)
        else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { location, .. } = operand.as_mut() else {
            unreachable!()
        };
        *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
    });
}

fn assert_target_operation_error(mutate: impl FnOnce(&mut TargetOperation)) {
    assert_eq!(
        candidate_error(|target| mutate(&mut target.functions[0].operation)),
        StraightLineIntegerExactCastParameterTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
