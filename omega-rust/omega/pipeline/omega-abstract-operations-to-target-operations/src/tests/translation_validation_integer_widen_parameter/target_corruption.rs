use super::*;
use omega_target_operations::TerminalPsiProvenance;

#[test]
fn integer_widen_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.operations.clear()),
        StraightLineIntegerWidenParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| target.functions[0].provenance.edges.clear()),
        StraightLineIntegerWidenParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerWidenParameterTranslationError::TargetProvenance
    );
}

#[test]
fn integer_widen_outer_target_expression_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { psi_edge, .. } = operation else {
            unreachable!()
        };
        *psi_edge = EdgeId::new(44_100).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { source_value, .. } = operation else {
            unreachable!()
        };
        *source_value = ValueId::new(44_101).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetOperation::ReturnIntegerExpression { scalar_type, .. } = operation else {
            unreachable!()
        };
        *scalar_type = integer_type(IntegerSign::Unsigned, 64);
    });
    assert_target_operation_error(|operation| {
        *operation = TargetOperation::ReturnIntegerParameter {
            psi_edge: EdgeId::new(3_004).unwrap(),
            source_value: ValueId::new(4_301).unwrap(),
            scalar_type: integer_type(IntegerSign::Signed, 64),
            parameter_index: 0,
            location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
        };
    });
}

#[test]
fn integer_widen_nested_target_expression_corruption_fails_closed() {
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerWiden { psi_operation, .. } = expression(operation)
        else {
            unreachable!()
        };
        *psi_operation = OperationId::new(44_110).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerWiden { source_type, .. } = expression(operation)
        else {
            unreachable!()
        };
        *source_type = integer_type(IntegerSign::Unsigned, 16);
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerWiden { operand, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { source_value, .. } = operand.as_mut() else {
            unreachable!()
        };
        *source_value = ValueId::new(44_111).unwrap();
    });
    assert_target_operation_error(|operation| {
        let TargetIntegerExpression::IntegerWiden { operand, .. } = expression(operation) else {
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
        let TargetIntegerExpression::IntegerWiden { operand, .. } = expression(operation) else {
            unreachable!()
        };
        let TargetIntegerExpression::Parameter { location, .. } = operand.as_mut() else {
            unreachable!()
        };
        *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
    });
    assert_target_operation_error(|operation| {
        let expression = expression(operation);
        let TargetIntegerExpression::IntegerWiden {
            psi_operation,
            operand,
            ..
        } = expression
        else {
            unreachable!()
        };
        *expression = TargetIntegerExpression::BitwiseNot {
            psi_operation: *psi_operation,
            operand: operand.clone(),
        };
    });
}

fn assert_target_operation_error(mutate: impl FnOnce(&mut TargetOperation)) {
    assert_eq!(
        candidate_error(|target| mutate(&mut target.functions[0].operation)),
        StraightLineIntegerWidenParameterTranslationError::TargetOperation
    );
}

fn expression(operation: &mut TargetOperation) -> &mut TargetIntegerExpression {
    let TargetOperation::ReturnIntegerExpression { expression, .. } = operation else {
        unreachable!()
    };
    expression
}
