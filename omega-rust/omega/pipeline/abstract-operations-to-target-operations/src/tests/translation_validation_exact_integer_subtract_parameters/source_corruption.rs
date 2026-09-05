use super::{integer_type, leaf_error};
use crate::StraightLineExactIntegerSubtractParametersTranslationError;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId};

#[test]
fn exact_integer_subtract_source_identity_type_and_obligation_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(
            |function| super::super::scalar_result_mut(function).scalar_type = ScalarType::Boolean
        ),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::ExactIntegerSubtract { result, .. } =
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
        StraightLineExactIntegerSubtractParametersTranslationError::SourceSubtractResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerSubtract { left, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(51_500).unwrap();
        }),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerSubtract { right, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(51_501).unwrap();
        }),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerSubtract { obligation, .. } =
                &mut function.operations[0]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(51_502).unwrap();
        }),
        StraightLineExactIntegerSubtractParametersTranslationError::TargetOperation
    );
    assert_eq!(
        leaf_error(|function| set_integer_type(function, integer_type(IntegerSign::Signed, 24))),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceParameterShape
    );
    assert_eq!(
        leaf_error(|function| set_integer_type(function, IntegerType::address(64).unwrap())),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceOperandTypeMismatch
    );
}

#[test]
fn exact_integer_subtract_source_semantics_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| replace_subtract(function, false)),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| replace_subtract(function, true)),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceReturnLink
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
        StraightLineExactIntegerSubtractParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineExactIntegerSubtractParametersTranslationError::SourceOperationRoster
    );
}

fn replace_subtract(function: &mut AbstractFunction, saturating: bool) {
    let AbstractOperation::ExactIntegerSubtract {
        psi_operation,
        result,
        scalar_type,
        left,
        right,
        ..
    } = function.operations[0]
    else {
        unreachable!()
    };
    function.operations[0] = if saturating {
        AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
    } else {
        AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
    };
}

fn set_integer_type(function: &mut AbstractFunction, scalar_type: IntegerType) {
    for parameter in &mut function.parameters {
        parameter.scalar_type = ScalarType::Integer(scalar_type);
    }
    let AbstractOperation::ExactIntegerSubtract {
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
