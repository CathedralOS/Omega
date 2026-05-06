use crate::diagnostics::Diagnostic;
use crate::ir::expression::{BinaryOperator, Expression};
use crate::ir::statement::TransitionGuard;
use crate::ir::types::TypeConstraint;
use crate::proof::obligations::{BoundedTransitionArgumentObligation, ProofObligation, ProofPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    minimum: i64,
    maximum: i64,
}

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for obligation in &proof_plan.obligations {
        let ProofObligation::BoundedTransitionArgument(obligation) = obligation else {
            continue;
        };

        check_bounded_transition_argument(obligation, &mut diagnostics);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_bounded_transition_argument(
    obligation: &BoundedTransitionArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target_range) = integer_range_from_constraints(&obligation.constraints) else {
        return;
    };
    let Some(mut argument_range) = integer_range_for_argument(obligation) else {
        diagnostics.push(cannot_prove_bounded_handoff(obligation, target_range));
        return;
    };

    argument_range = apply_guard(argument_range, &obligation.argument, &obligation.guard);

    if argument_range.minimum < target_range.minimum
        || argument_range.maximum > target_range.maximum
    {
        diagnostics.push(cannot_prove_bounded_handoff(obligation, target_range));
    }
}

fn integer_range_for_argument(
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<IntegerRange> {
    match &obligation.argument {
        Expression::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(&obligation.argument_constraints),
    }
}

fn integer_range_from_constraints(constraints: &[TypeConstraint]) -> Option<IntegerRange> {
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            return None;
        };

        Some(IntegerRange {
            minimum: integer_literal(minimum)?,
            maximum: integer_literal(maximum)?,
        })
    })
}

fn integer_literal(expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        _ => None,
    }
}

fn apply_guard(
    range: IntegerRange,
    argument: &Expression,
    guard: &TransitionGuard,
) -> IntegerRange {
    match guard {
        TransitionGuard::Always => range,
        TransitionGuard::When(condition) => apply_condition(range, argument, condition),
    }
}

fn apply_condition(
    range: IntegerRange,
    argument: &Expression,
    condition: &Expression,
) -> IntegerRange {
    let Expression::Binary(binary) = condition else {
        return range;
    };

    if binary.operator == BinaryOperator::And {
        let range = apply_condition(range, argument, &binary.left);
        return apply_condition(range, argument, &binary.right);
    }

    if binary.left == *argument {
        return apply_right_literal_guard(range, binary.operator, &binary.right);
    }

    if binary.right == *argument {
        return apply_left_literal_guard(range, &binary.left, binary.operator);
    }

    range
}

fn apply_right_literal_guard(
    mut range: IntegerRange,
    operator: BinaryOperator,
    right: &Expression,
) -> IntegerRange {
    let Some(value) = integer_literal(right) else {
        return range;
    };

    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.minimum = range.minimum.max(value.saturating_add(1)),
        BinaryOperator::GreaterOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Less => range.maximum = range.maximum.min(value.saturating_sub(1)),
        BinaryOperator::LessOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::NotEqual
        | BinaryOperator::Or => {}
    }

    range
}

fn apply_left_literal_guard(
    mut range: IntegerRange,
    left: &Expression,
    operator: BinaryOperator,
) -> IntegerRange {
    let Some(value) = integer_literal(left) else {
        return range;
    };

    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.maximum = range.maximum.min(value.saturating_sub(1)),
        BinaryOperator::GreaterOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Less => range.minimum = range.minimum.max(value.saturating_add(1)),
        BinaryOperator::LessOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::NotEqual
        | BinaryOperator::Or => {}
    }

    range
}

fn cannot_prove_bounded_handoff(
    obligation: &BoundedTransitionArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.argument.display_name(),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}
