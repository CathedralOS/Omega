use crate::legalization::catalog::ScalarLegalizationValidatorKind;

use super::super::leaf::{
    replay_active_resident_bridge_chain_shape, replay_active_resident_chain_shape,
};
use super::super::shared::*;

pub(crate) fn scalar_validator_accepts(
    validator: ScalarLegalizationValidatorKind,
    when_true: &TargetIntegerControl,
    when_false: &TargetIntegerControl,
) -> bool {
    match validator {
        ScalarLegalizationValidatorKind::Immediate => [when_true, when_false].iter().all(|arm| {
            matches!(
                arm,
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::Immediate { .. },
                    ..
                }
            )
        }),
        ScalarLegalizationValidatorKind::EntryParameter => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm,
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::Parameter { .. },
                        ..
                    }
                )
            })
        }
        ScalarLegalizationValidatorKind::ExactAddImmediate => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm,
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::ExactAdd { left, right, .. },
                        ..
                    } if independently_immediate(left) && independently_immediate(right)
                )
            })
        }
        ScalarLegalizationValidatorKind::ExactSubtractImmediate => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm,
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::ExactSubtract { left, right, .. },
                        ..
                    } if independently_immediate(left) && independently_immediate(right)
                )
            })
        }
        ScalarLegalizationValidatorKind::WidenedU8ExactAddImmediate => [when_true, when_false]
            .iter()
            .all(|arm| independently_widened_binary(arm, true)),
        ScalarLegalizationValidatorKind::WidenedU8ExactSubtractImmediate => [when_true, when_false]
            .iter()
            .all(|arm| independently_widened_binary(arm, false)),
        ScalarLegalizationValidatorKind::ActiveResidentExactAddChain => matches!(
            (when_true, when_false),
            (
                TargetIntegerControl::Return { expression, .. },
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::Immediate { .. },
                    ..
                }
            ) if replay_active_resident_chain_shape(expression)
        ),
        ScalarLegalizationValidatorKind::ActiveResidentExactAddBridgeChain => matches!(
            (when_true, when_false),
            (
                TargetIntegerControl::Return { expression, .. },
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::Immediate { .. },
                    ..
                }
            ) if replay_active_resident_bridge_chain_shape(expression)
        ),
    }
}

fn independently_widened_binary(control: &TargetIntegerControl, add: bool) -> bool {
    let TargetIntegerControl::Return {
        expression:
            TargetIntegerExpression::IntegerWiden {
                source_type,
                operand,
                ..
            },
        ..
    } = control
    else {
        return false;
    };
    if source_type.sign() != IntegerSign::Unsigned || source_type.bits() != 8 {
        return false;
    }
    match operand.as_ref() {
        TargetIntegerExpression::ExactAdd { left, right, .. } if add => {
            independently_immediate(left) && independently_immediate(right)
        }
        TargetIntegerExpression::ExactSubtract { left, right, .. } if !add => {
            independently_immediate(left) && independently_immediate(right)
        }
        _ => false,
    }
}

fn independently_immediate(expression: &TargetIntegerExpression) -> bool {
    matches!(expression, TargetIntegerExpression::Immediate { .. })
}
