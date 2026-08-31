use super::{integer_type, leaf_error};
use crate::StraightLineExactIntegerAddParametersTranslationError;
use omega_abstract_operations::{AbstractFunction, AbstractOperation};
use psi_core::{IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, ValueId};

#[test]
fn exact_integer_add_source_identity_type_and_obligation_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| function.parameters.clear()),
        StraightLineExactIntegerAddParametersTranslationError::SourceParameters
    );
    assert_eq!(
        leaf_error(
            |function| super::super::scalar_result_mut(function).scalar_type = ScalarType::Boolean
        ),
        StraightLineExactIntegerAddParametersTranslationError::SourceResult
    );
    assert_eq!(
        leaf_error(|function| {
            let parameter = function.parameters[0].value;
            let AbstractOperation::ExactIntegerAdd { result, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *result = parameter;
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = parameter;
        }),
        StraightLineExactIntegerAddParametersTranslationError::SourceAddResultRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerAdd { left, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *left = ValueId::new(51_500).unwrap();
        }),
        StraightLineExactIntegerAddParametersTranslationError::SourceLeftOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerAdd { right, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *right = ValueId::new(51_501).unwrap();
        }),
        StraightLineExactIntegerAddParametersTranslationError::SourceRightOperandLink
    );
    assert_eq!(
        leaf_error(|function| {
            function.parameters[1].scalar_type =
                ScalarType::Integer(integer_type(IntegerSign::Unsigned, 32));
        }),
        StraightLineExactIntegerAddParametersTranslationError::SourceOperandTypeMismatch
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::ExactIntegerAdd { obligation, .. } = &mut function.operations[0]
            else {
                unreachable!()
            };
            *obligation = ObligationId::new(51_502).unwrap();
        }),
        StraightLineExactIntegerAddParametersTranslationError::TargetOperation
    );
    assert_eq!(
        leaf_error(|function| set_integer_type(function, integer_type(IntegerSign::Signed, 24))),
        StraightLineExactIntegerAddParametersTranslationError::SourceParameterShape
    );
}

#[test]
fn exact_integer_add_source_semantics_return_and_cleanup_corruption_fails_closed() {
    assert_eq!(
        leaf_error(|function| replace_add(function, false)),
        StraightLineExactIntegerAddParametersTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| replace_add(function, true)),
        StraightLineExactIntegerAddParametersTranslationError::SourceOperationRoster
    );
    assert_eq!(
        leaf_error(|function| {
            let AbstractOperation::Return { value, .. } = &mut function.operations[1] else {
                unreachable!()
            };
            *value = function.parameters[0].value;
        }),
        StraightLineExactIntegerAddParametersTranslationError::SourceReturnLink
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
                PlaceId::new(51_503).unwrap(),
            ));
        }),
        StraightLineExactIntegerAddParametersTranslationError::SourceCleanup
    );
    assert_eq!(
        leaf_error(|function| function.operations.swap(0, 1)),
        StraightLineExactIntegerAddParametersTranslationError::SourceOperationRoster
    );
}

fn replace_add(function: &mut AbstractFunction, saturating: bool) {
    let AbstractOperation::ExactIntegerAdd {
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
        AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
    } else {
        AbstractOperation::WrappingIntegerAdd {
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
    let AbstractOperation::ExactIntegerAdd {
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
