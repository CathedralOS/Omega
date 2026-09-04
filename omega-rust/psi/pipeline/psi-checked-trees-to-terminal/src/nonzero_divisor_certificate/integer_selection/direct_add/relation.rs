//! Canonical fixed-integer carrier-bound add recognition.

use psi_core::{IntegerMathTerm, IntegerType, Proposition, ScalarTerm};

use super::super::dispatch;

pub(super) struct DirectAddRelation {
    pub(super) integer_type: IntegerType,
    pub(super) left: ScalarTerm,
    pub(super) right: ScalarTerm,
    pub(super) target: ScalarTerm,
    pub(super) lower: bool,
}

pub(super) fn classify(goal: &Proposition) -> Option<DirectAddRelation> {
    let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let (sum, carrier_bound, lower) = match (goal_left, goal_right) {
        (IntegerMathTerm::Add(_, _), IntegerMathTerm::IntegerLiteral(bound)) => {
            (goal_left, *bound, false)
        }
        (IntegerMathTerm::IntegerLiteral(bound), IntegerMathTerm::Add(_, _)) => {
            (goal_right, *bound, true)
        }
        _ => return None,
    };
    let IntegerMathTerm::Add(left, right) = sum else {
        unreachable!("classified direct mathematical addition")
    };
    let integer_type = direct_integer_type(left, right)?;
    let expected = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if integer_type.carrier() != psi_core::IntegerCarrier::Fixed
        || carrier_bound.as_integer_value(integer_type) != Some(expected)
    {
        return None;
    }
    let left = dispatch::lower_add_math_leaf(left, integer_type)?;
    let right = dispatch::lower_add_math_leaf(right, integer_type)?;
    let target = ScalarTerm::exact_integer_add(integer_type, left.clone(), right.clone()).ok()?;
    Some(DirectAddRelation {
        integer_type,
        left,
        right,
        target,
        lower,
    })
}

fn direct_integer_type(left: &IntegerMathTerm, right: &IntegerMathTerm) -> Option<IntegerType> {
    match (left, right) {
        (
            IntegerMathTerm::MathValue { source_type, .. },
            IntegerMathTerm::MathValue {
                source_type: right_type,
                ..
            },
        ) if source_type == right_type => Some(*source_type),
        (IntegerMathTerm::MathValue { source_type, .. }, IntegerMathTerm::IntegerLiteral(_))
        | (IntegerMathTerm::IntegerLiteral(_), IntegerMathTerm::MathValue { source_type, .. }) => {
            Some(*source_type)
        }
        _ => None,
    }
}
