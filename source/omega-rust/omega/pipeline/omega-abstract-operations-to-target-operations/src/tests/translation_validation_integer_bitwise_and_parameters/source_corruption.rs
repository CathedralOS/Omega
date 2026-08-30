use super::*;

#[test]
fn integer_bitwise_and_source_identity_and_type_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(|function| scalar_result_mut(function).scalar_type = ScalarType::Boolean),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerBitwiseAnd { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceAndResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseAnd { left, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(45_500).unwrap();
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseAnd { right, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(45_501).unwrap();
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerBitwiseAnd { scalar_type, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *scalar_type = integer_type(IntegerSign::Unsigned, 32);
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceResult
    );
    for invalid in [
        integer_type(IntegerSign::Signed, 24),
        IntegerType::address(64).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| set_integer_type(function, invalid)),
            StraightLineIntegerBitwiseAndParametersTranslationError::SourceAndTypeMismatch
        );
    }
}

#[test]
fn integer_bitwise_and_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceReturnLink
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
                PlaceId::new(45_502).unwrap(),
            ));
        }),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerBitwiseAndParametersTranslationError::SourceOperationRoster
    );
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::IntegerBitwiseAnd {
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
