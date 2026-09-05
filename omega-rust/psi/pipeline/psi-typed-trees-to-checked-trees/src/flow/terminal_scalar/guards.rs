//! Exhaustive Boolean pairs use the already selected, effect-free guards.

use psi_checked_trees::{
    CheckedBooleanExpression, CheckedScalarExpression, CheckedScalarExpressionPlans,
    CheckedScalarExpressionRole,
};

#[cfg(test)]
mod tests;

pub(super) fn complementary(
    expressions: &CheckedScalarExpressionPlans,
    state: psi_symbols::SymbolHandle,
    statement_ordinal: u32,
) -> bool {
    let Some(next) = statement_ordinal.checked_add(1) else {
        return false;
    };
    let guard = |ordinal| {
        let mut selected = expressions.expressions.iter().filter(|expression| {
            expression.state == state
                && expression.statement_ordinal == ordinal
                && expression.role == CheckedScalarExpressionRole::Guard
        });
        let expression = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        match &expression.expression {
            CheckedScalarExpression::Boolean(expression) => Some(expression.as_ref()),
            _ => None,
        }
    };
    let (Some(left), Some(right)) = (guard(statement_ordinal), guard(next)) else {
        return false;
    };
    let (left, left_polarity) = base(left);
    let (right, right_polarity) = base(right);
    left == right && left_polarity != right_polarity
}

fn base(mut expression: &CheckedBooleanExpression) -> (&CheckedBooleanExpression, bool) {
    let mut polarity = true;
    loop {
        match expression {
            CheckedBooleanExpression::Not(operand) => {
                expression = operand;
                polarity = !polarity;
            }
            CheckedBooleanExpression::Equal { left, right } => {
                match (left.as_ref(), right.as_ref()) {
                    (CheckedBooleanExpression::Constant(value), operand)
                    | (operand, CheckedBooleanExpression::Constant(value)) => {
                        expression = operand;
                        polarity = polarity == *value;
                    }
                    _ => return (expression, polarity),
                }
            }
            _ => return (expression, polarity),
        }
    }
}
