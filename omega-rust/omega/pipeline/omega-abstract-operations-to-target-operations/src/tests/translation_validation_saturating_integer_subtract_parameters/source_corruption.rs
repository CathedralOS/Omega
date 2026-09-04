use super::*;

#[test]
fn saturating_integer_subtract_source_identity_and_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| scalar_result_mut(function).scalar_type = ScalarType::Boolean),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::SaturatingIntegerSubtract { result, .. } =
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
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceSubtractResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerSubtract { left, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(49_600).unwrap();
        }),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerSubtract { right, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(49_601).unwrap();
        }),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::SaturatingIntegerSubtract { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Unsigned, 32);
        }),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| set_integer_type(function, integer_type(IntegerSign::Signed, 24))),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceParameterShape
    );
}

#[test]
fn saturating_integer_subtract_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceReturnLink
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
                PlaceId::new(49_602).unwrap(),
            ));
        }),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineSaturatingIntegerSubtractParametersTranslationError::SourceOperationRoster
    );
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::SaturatingIntegerSubtract {
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
