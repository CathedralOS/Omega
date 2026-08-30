use super::*;

#[test]
fn exact_cast_source_links_and_type_relation_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::IntegerExactCast { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineIntegerExactCastParameterTranslationError::SourceCastResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::IntegerExactCast { operand, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *operand = ValueId::new(45_000).unwrap();
        }),
        StraightLineIntegerExactCastParameterTranslationError::SourceOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[0].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Signed, 64));
        }),
        StraightLineIntegerExactCastParameterTranslationError::SourceOperandTypeMismatch
    );
    for invalid_target in [
        integer_type(IntegerSign::Unsigned, 64),
        integer_type(IntegerSign::Unsigned, 128),
        integer_type(IntegerSign::Unsigned, 24),
        IntegerType::address(64).unwrap(),
    ] {
        assert_eq!(
            leaf_error(|function| set_target_type(function, invalid_target)),
            StraightLineIntegerExactCastParameterTranslationError::SourceCastTypeMismatch
        );
    }
}

#[test]
fn exact_cast_source_return_and_shape_corruption_fail_closed() {
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineIntegerExactCastParameterTranslationError::SourceReturnLink
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
                PlaceId::new(45_001).unwrap(),
            ));
        }),
        StraightLineIntegerExactCastParameterTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineIntegerExactCastParameterTranslationError::SourceOperationRoster
    );
}

fn set_target_type(function: &mut AbstractFunction, target_type: IntegerType) {
    let AbstractOperation::IntegerExactCast {
        target_type: declared,
        ..
    } = &mut function.operations[0]
    else {
        unreachable!()
    };
    *declared = target_type;
    let AbstractOperation::Return { scalar_type, .. } = &mut function.operations[1] else {
        unreachable!()
    };
    *scalar_type = ScalarType::Integer(target_type);
    let AbstractFunctionResult::Scalar(result) = &mut function.result else {
        unreachable!()
    };
    result.scalar_type = ScalarType::Integer(target_type);
}
