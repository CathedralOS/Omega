use super::*;

#[test]
fn integer_bitwise_or_source_identity_and_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| scalar_result_mut(function).scalar_type = ScalarType::Boolean),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerBitwiseOr { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceOrResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseOr { left, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(46_500).unwrap();
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseOr { right, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(46_501).unwrap();
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseOr { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Unsigned, 32);
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceResult
    );
    for invalid in [
        integer_type(IntegerSign::Signed, 24),
        IntegerType::address(64).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| set_integer_type(function, invalid)),
            StraightLineIntegerBitwiseOrParametersTranslationError::SourceOrTypeMismatch
        );
    }
}

#[test]
fn integer_bitwise_or_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceReturnLink
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
                PlaceId::new(46_502).unwrap(),
            ));
        }),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerBitwiseOrParametersTranslationError::SourceOperationRoster
    );
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::IntegerBitwiseOr {
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
    let AbstractFunctionResult::Scalar(result) = &mut function.result else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Integer(scalar_type);
}
