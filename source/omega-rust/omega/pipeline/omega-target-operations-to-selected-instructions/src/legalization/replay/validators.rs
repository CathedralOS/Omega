//! Validator-only recognition of one proposed scalar legalization recipe.
//!
//! This deliberately does not call the producer matcher. The adjacent catalog
//! supplies identity and closed shape metadata; replay reconstructs whether
//! the raw target expression belongs to that form independently.

use crate::legalization::catalog::ScalarLegalizationValidatorKind;

use super::leaf::replay_active_resident_chain_shape;
use super::shared::*;

pub(super) fn validator_accepts(
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
