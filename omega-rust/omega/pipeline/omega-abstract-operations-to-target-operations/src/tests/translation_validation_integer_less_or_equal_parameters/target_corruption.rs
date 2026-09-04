use super::*;

#[test]
fn integer_less_or_equal_target_corruption_fails_closed() {
    assert_eq!(
        candidate_error(|target| {
            target.functions[0].provenance = TerminalPsiProvenance::default();
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetProvenance
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { scalar_type, .. } = expression else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Signed, 64);
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { source_value, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            *source_value = ValueId::new(49_999).unwrap();
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter {
                location: left_location,
                ..
            } = left.as_ref()
            else {
                unreachable!()
            };
            let left_location = *left_location;
            let TargetIntegerExpression::Parameter {
                location: right_location,
                ..
            } = right.as_mut()
            else {
                unreachable!()
            };
            *right_location = left_location;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { left, right, .. } = expression else {
                unreachable!()
            };
            std::mem::swap(left, right);
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual { left, .. } = expression else {
                unreachable!()
            };
            let TargetIntegerExpression::Parameter {
                parameter_index, ..
            } = left.as_mut()
            else {
                unreachable!()
            };
            *parameter_index = 1;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
    assert_eq!(
        candidate_error(|target| {
            let TargetOperation::ReturnBooleanExpression { expression, .. } =
                &mut target.functions[0].operation
            else {
                unreachable!()
            };
            let TargetBooleanExpression::IntegerLessOrEqual {
                psi_operation,
                scalar_type,
                left,
                right,
            } = expression
            else {
                unreachable!()
            };
            *expression = TargetBooleanExpression::IntegerLessThan {
                psi_operation: *psi_operation,
                scalar_type: *scalar_type,
                left: left.clone(),
                right: right.clone(),
            };
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::TargetOperation
    );
}
