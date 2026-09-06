use crate::legalization::catalog::{
    LEGALIZATION_FORMS, LegalizationFormDescriptor, LegalizationProducerMatcherKind,
    LegalizationShapeConstraints, ScalarConditionShape, ScalarLegalizationMatcherKind,
};

use super::super::shared::*;

pub(crate) fn match_scalar_form(
    condition: ScalarConditionShape,
    when_true: &TargetIntegerControl,
    when_false: &TargetIntegerControl,
) -> Option<&'static LegalizationFormDescriptor> {
    let mut matches = LEGALIZATION_FORMS.iter().filter(|descriptor| {
        if !match descriptor.constraints {
            LegalizationShapeConstraints::Scalar(constraints) => constraints.condition == condition,
            LegalizationShapeConstraints::ScalarSequence => {
                condition == ScalarConditionShape::DirectBooleanParameter
            }
            _ => false,
        } {
            return false;
        }
        let LegalizationProducerMatcherKind::Scalar(matcher) = descriptor.producer_matcher else {
            return false;
        };
        matcher_accepts(matcher, when_true, when_false)
    });
    let matched = matches.next()?;
    matches.next().is_none().then_some(matched)
}

fn matcher_accepts(
    matcher: ScalarLegalizationMatcherKind,
    when_true: &TargetIntegerControl,
    when_false: &TargetIntegerControl,
) -> bool {
    match matcher {
        ScalarLegalizationMatcherKind::Immediate => [when_true, when_false].iter().all(|arm| {
            matches!(
                arm,
                TargetIntegerControl::Return {
                    expression: TargetIntegerExpression::Immediate { .. },
                    ..
                }
            )
        }),
        ScalarLegalizationMatcherKind::EntryParameter => {
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
        ScalarLegalizationMatcherKind::ExactAddImmediate => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm,
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::ExactAdd { left, right, .. },
                        ..
                    } if immediate_pair(left, right)
                )
            })
        }
        ScalarLegalizationMatcherKind::ExactSubtractImmediate => {
            [when_true, when_false].iter().all(|arm| {
                matches!(
                    arm,
                    TargetIntegerControl::Return {
                        expression: TargetIntegerExpression::ExactSubtract { left, right, .. },
                        ..
                    } if immediate_pair(left, right)
                )
            })
        }
        ScalarLegalizationMatcherKind::WidenedU8ExactAddImmediate => [when_true, when_false]
            .iter()
            .all(|arm| widened_immediate_binary(arm, true)),
        ScalarLegalizationMatcherKind::WidenedU8ExactSubtractImmediate => [when_true, when_false]
            .iter()
            .all(|arm| widened_immediate_binary(arm, false)),
        ScalarLegalizationMatcherKind::ExactIntegerSequence => matches!(
            (when_true, when_false),
            (TargetIntegerControl::Return { expression, .. },
             TargetIntegerControl::Return { expression: TargetIntegerExpression::Immediate { .. }, .. })
            if matches!(expression, TargetIntegerExpression::ExactAdd { .. } | TargetIntegerExpression::ExactSubtract { .. })
                && crate::legalization::integer_sequence_input::expression_shape(expression)
        ),
    }
}

fn widened_immediate_binary(control: &TargetIntegerControl, add: bool) -> bool {
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
    if *source_type != semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8")
    {
        return false;
    }
    match operand.as_ref() {
        TargetIntegerExpression::ExactAdd { left, right, .. } if add => immediate_pair(left, right),
        TargetIntegerExpression::ExactSubtract { left, right, .. } if !add => {
            immediate_pair(left, right)
        }
        _ => false,
    }
}

fn immediate_pair(left: &TargetIntegerExpression, right: &TargetIntegerExpression) -> bool {
    matches!(left, TargetIntegerExpression::Immediate { .. })
        && matches!(right, TargetIntegerExpression::Immediate { .. })
}
