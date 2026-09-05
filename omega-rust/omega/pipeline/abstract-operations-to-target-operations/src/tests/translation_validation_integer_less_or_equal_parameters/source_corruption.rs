use super::*;

#[test]
fn integer_less_or_equal_source_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerLessOrEqual { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceLessOrEqualResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type = ScalarType::Boolean;
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *scalar_type = ScalarType::Integer(integer_type(IntegerSign::Signed, 32));
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let (psi_operation, result, left, right) = match &function.operations[0] {
                AbstractOperation::IntegerLessOrEqual {
                    psi_operation,
                    result,
                    left,
                    right,
                } => (*psi_operation, *result, *left, *right),
                _ => unreachable!(),
            };
            function.operations[0] = AbstractOperation::IntegerLessThan {
                psi_operation,
                result,
                left,
                right,
            };
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(4_200).unwrap(),
            ));
        }),
        StraightLineIntegerLessOrEqualParametersTranslationError::SourceCleanup
    );
}
