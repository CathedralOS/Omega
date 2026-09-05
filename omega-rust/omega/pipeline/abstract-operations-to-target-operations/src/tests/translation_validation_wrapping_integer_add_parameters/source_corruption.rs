use super::*;

#[test]
fn wrapping_integer_add_source_identity_and_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| scalar_result_mut(function).scalar_type = ScalarType::Boolean),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::WrappingIntegerAdd { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceAddResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerAdd { left, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(48_500).unwrap();
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerAdd { right, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(48_501).unwrap();
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::WrappingIntegerAdd { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Unsigned, 32);
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            set_integer_type(function, integer_type(IntegerSign::Signed, 24))
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceParameterShape
    );
}

#[test]
fn wrapping_integer_add_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceReturnLink
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
                PlaceId::new(48_502).unwrap(),
            ));
        }),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineWrappingIntegerAddParametersTranslationError::SourceOperationRoster
    );
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::WrappingIntegerAdd {
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
