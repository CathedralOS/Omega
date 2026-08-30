use super::*;

#[test]
fn integer_bitwise_not_target_provenance_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.operations.clear();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance.edges.clear();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetProvenance
    );
}

#[test]
fn integer_bitwise_not_target_expression_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { psi_edge, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *psi_edge = EdgeId::new(43_000).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { scalar_type, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Signed, 64);
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { source_value, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *source_value = ValueId::new(43_001).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { psi_operation, .. } = expression else {
                unreachable!()
            };
            *psi_operation = OperationId::new(43_002).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { operand, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter { source_value, .. } = operand.as_mut() else {
                unreachable!()
            };
            *source_value = ValueId::new(43_003).unwrap();
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { operand, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter {
                parameter_index, ..
            } = operand.as_mut()
            else {
                unreachable!()
            };
            *parameter_index = 1;
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot { operand, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter { location, .. } = operand.as_mut() else {
                unreachable!()
            };
            *location = ScalarParameterLocation::IncomingStack { byte_offset: 0 };
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnIntegerExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetIntegerExpression::BitwiseNot {
                psi_operation,
                operand,
            } = expression
            else {
                unreachable!()
            };
            *expression = TargetIntegerExpression::IntegerWiden {
                psi_operation: *psi_operation,
                source_type: integer_type(IntegerSign::Unsigned, 32),
                operand: operand.clone(),
            };
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].operation = TargetOperation::ReturnIntegerParameter {
                psi_edge: EdgeId::new(3_004).unwrap(),
                source_value: ValueId::new(4_201).unwrap(),
                scalar_type: integer_type(IntegerSign::Unsigned, 64),
                parameter_index: 0,
                location: ScalarParameterLocation::Register(MachineRegister::X86Rdi),
            };
        }),
        StraightLineIntegerBitwiseNotParameterTranslationError::TargetOperation
    );
}
