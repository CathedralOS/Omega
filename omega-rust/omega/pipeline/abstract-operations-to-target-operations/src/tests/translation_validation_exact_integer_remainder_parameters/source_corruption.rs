use super::{integer_type, leaf_error};
use crate::StraightLineExactIntegerRemainderParametersTranslationError;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId};

#[test]
fn exact_integer_remainder_source_identity_type_and_obligation_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(
            |function| super::super::scalar_result_mut(function).scalar_type = ScalarType::Boolean
        ),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::ExactIntegerRemainder { result, .. } =
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
        StraightLineExactIntegerRemainderParametersTranslationError::SourceRemainderResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerRemainder { left, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(51_500).unwrap();
        }),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerRemainder { right, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(51_501).unwrap();
        }),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerRemainder { obligation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(51_502).unwrap();
        }),
        StraightLineExactIntegerRemainderParametersTranslationError::TargetOperation
    );
    assert_eq!(
        leaf_error(|function| set_integer_type(function, integer_type(IntegerSign::Signed, 24))),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceParameterShape
    );
    assert_eq!(
        leaf_error(|function| set_integer_type(function, IntegerType::address(64).unwrap())),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceOperandTypeMismatch
    );
}

#[test]
fn exact_integer_remainder_source_semantics_return_and_cleanup_corruption_fails_closed() {
    for substitute in 1..=14 {
        assert_eq!(
            leaf_error(|function| replace_remainder(function, substitute)),
            StraightLineExactIntegerRemainderParametersTranslationError::SourceOperationRoster
        );
    }
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceReturnLink
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
                PlaceId::new(51_503).unwrap(),
            ));
        }),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineExactIntegerRemainderParametersTranslationError::SourceOperationRoster
    );
}

fn replace_remainder(function: &mut AbstractFunction, substitute: u8) {
    let AbstractOperation::ExactIntegerRemainder {
        psi_operation,
        obligation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = function.operations[0]
    else {
        unreachable!()
    };
    function.operations[0] = match substitute {
        1 => AbstractOperation::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        2 => AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        3 => AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        4 => AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        5 => AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        6 => AbstractOperation::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        7 => AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        8 => AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        9 => AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        10 => AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        11 => AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        12 => AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        },
        13 => AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        14 => AbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        },
        _ => unreachable!(),
    };
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::ExactIntegerRemainder {
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
    super::super::scalar_result_mut(function).scalar_type = ScalarType::Integer(scalar_type);
}
