use super::*;

#[test]
fn wrapping_integer_subtract_source_identity_and_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| scalar_result_mut(function).scalar_type = ScalarType::Boolean),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::WrappingIntegerSubtract { result, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceSubtractResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerSubtract { left, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(49_500).unwrap();
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerSubtract { right, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(49_501).unwrap();
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerSubtract { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Unsigned, 32);
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            set_integer_type(function, integer_type(IntegerSign::Signed, 24))
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceParameterShape
    );
}

#[test]
fn wrapping_integer_subtract_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceReturnLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return {
                cleanup_actions, ..
            } = &mut function.operations[1]
            else {
                unreachable!()
            };
            cleanup_actions.push(psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
                PlaceId::new(49_502).unwrap(),
            ));
        }),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineWrappingIntegerSubtractParametersTranslationError::SourceOperationRoster
    );
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::WrappingIntegerSubtract {
        scalar_type: declared,
        ..
    } = &mut function.operations[0]
    else {
        unreachable!()
    };
    *declared = scalar_type;
    let AbstractOperation::Return {
        scalar_type: returned,
        ..
    } = &mut function.operations[1]
    else {
        unreachable!()
    };
    *returned = ScalarType::Integer(scalar_type);
    scalar_result_mut(function).scalar_type = ScalarType::Integer(scalar_type);
}
