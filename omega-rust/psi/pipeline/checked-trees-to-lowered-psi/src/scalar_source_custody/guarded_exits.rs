//! Ordered exits are checked against the entire authored suffix. A final case
//! remains a guard; only exact coverage of one unchanged subject removes the
//! need for an authored fallback.

use super::*;
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarGuardedExit};

pub(crate) fn complementary(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    ordinal: u32,
) -> Result<bool, LoweringError> {
    use checked_trees::{CheckedBooleanExpression as Boolean, CheckedScalarExpression};
    let guard = |statement| {
        let (source, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, CheckedScalarExpressionRole::Guard)
            .ok_or(LoweringError::Unsupported(
                "complementary fallback lost its pure source guard",
            ))?;
        crate::scalar_source_custody::validate_pure(checked, source, ScalarType::Boolean)?;
        let CheckedScalarExpression::Boolean(expression) = expression else {
            return unsupported("complementary fallback guard is not Boolean");
        };
        Ok(expression.as_ref())
    };
    fn base(mut expression: &Boolean) -> (&Boolean, bool) {
        let mut polarity = true;
        loop {
            match expression {
                Boolean::Not(operand) => {
                    expression = operand;
                    polarity = !polarity;
                }
                Boolean::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
                    (Boolean::Constant(value), operand) | (operand, Boolean::Constant(value)) => {
                        expression = operand;
                        polarity = polarity == *value;
                    }
                    _ => return (expression, polarity),
                },
                _ => return (expression, polarity),
            }
        }
    }
    let next = ordinal
        .checked_add(1)
        .ok_or(LoweringError::Unsupported("fallback ordinal overflow"))?;
    let (left, left_polarity) = base(guard(ordinal)?);
    let (right, right_polarity) = base(guard(next)?);
    Ok(left == right && left_polarity != right_polarity)
}

pub(crate) fn validate(
    checked: &CheckedTrees,
    state_symbol: symbols::SymbolHandle,
    arms: arena::HandleSpan<CheckedScalarGuardedExit>,
    fallback: Option<&CheckedScalarBranchDestination>,
) -> Result<usize, LoweringError> {
    let (_, state) = authored_state(checked, state_symbol)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(arms)
        .ok_or(LoweringError::Unsupported(
            "ordered scalar guard roster is stale",
        ))?;
    let first = arms.first().ok_or(LoweringError::Unsupported(
        "ordered scalar guard roster is empty",
    ))?;
    let prefix = first.guard_statement_ordinal as usize;
    let tail = statements
        .get(prefix..)
        .ok_or(LoweringError::Unsupported("ordered scalar tail is absent"))?;
    if tail.len() != arms.len() + usize::from(fallback.is_some()) {
        return unsupported("ordered scalar guards omit or add an authored exit");
    }
    let mut coverage = Vec::new();
    let mut coverage_owner = None;
    let mut coverage_subject = None;
    let complementary_pair = fallback.is_none()
        && arms.len() == 2
        && complementary(checked, state_symbol, first.guard_statement_ordinal)?;
    for (index, (arm, statement)) in arms.iter().zip(tail).enumerate() {
        let ordinal = u32::try_from(prefix + index)
            .map_err(|_| LoweringError::Unsupported("ordered scalar ordinal overflow"))?;
        if arm.guard_statement_ordinal != ordinal {
            return unsupported("ordered scalar guards changed authored order");
        }
        let StatementNode::Transition(transition) = statement else {
            return unsupported("ordered scalar guard replaced an authored statement");
        };
        let TransitionGuardNode::When(expression) = transition.guard else {
            return unsupported("ordered scalar guard replaced an authored fallback");
        };
        if transition.continuation.is_valid() {
            return unsupported("ordered scalar guard omitted an authored continuation");
        }
        destination(checked, state_symbol, ordinal, &arm.destination)?;
        if fallback.is_none() && !complementary_pair {
            // A retained Boolean cannot assert coverage. First replay its exact
            // source read and selected operator, then recover the nominal case.
            let (binding, _) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(state_symbol, ordinal, CheckedScalarExpressionRole::Guard)
                .ok_or(LoweringError::Unsupported(
                    "exhaustive scalar guard has no pure source occurrence",
                ))?;
            validate_pure(checked, binding, ScalarType::Boolean)?;
            let expression = storage_reads::guard_subject(checked, expression);
            let (subject, case) = storage_reads::case_membership::authored(
                checked, state, expression,
            )?
            .ok_or(LoweringError::Unsupported(
                "scalar tail has neither authored fallback nor exact case coverage",
            ))?;
            let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression)
            else {
                return unsupported("exhaustive scalar guard lost its case expression");
            };
            let ExpressionNode::Name(selected) = checked.expression_table.expression(binary.right)
            else {
                return unsupported("exhaustive scalar guard lost its selected case");
            };
            let owner = checked.symbols.get(selected.symbol).parent;
            if coverage_subject.is_some_and(|prior| prior != subject)
                || coverage_owner.is_some_and(|prior| prior != owner)
            {
                return unsupported("scalar case coverage combines different subjects or sums");
            }
            coverage_subject = Some(subject);
            coverage_owner = Some(owner);
            coverage.push(case);
        }
    }
    if let Some(fallback) = fallback {
        let ordinal = u32::try_from(prefix + arms.len())
            .map_err(|_| LoweringError::Unsupported("scalar fallback ordinal overflow"))?;
        match tail.last() {
            Some(StatementNode::Expression(_)) => {}
            Some(StatementNode::Transition(transition))
                if transition.guard == TransitionGuardNode::Always
                    && !transition.continuation.is_valid() => {}
            _ => return unsupported("ordered scalar fallback is not authored unconditionally"),
        }
        destination(checked, state_symbol, ordinal, fallback)?;
    } else if !complementary_pair {
        let data = checked
            .data_definitions()
            .iter()
            .find(|data| Some(data.symbol) == coverage_owner)
            .ok_or(LoweringError::Unsupported(
                "scalar case coverage has no nominal owner",
            ))?;
        for member in checked.data_members(data) {
            let checked_trees::data::DataMember::Variant(case) = member else {
                return unsupported("scalar case coverage is not a closed sum");
            };
            let identity = case
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| case.name.as_str().to_owned());
            if !coverage.contains(&identity) {
                return unsupported("scalar case coverage omitted an authored case");
            }
        }
    }
    Ok(prefix)
}

fn destination(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    ordinal: u32,
    destination: &CheckedScalarBranchDestination,
) -> Result<(), LoweringError> {
    match destination {
        CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation: false,
        } if *statement_ordinal == ordinal => {
            let (_, source) = authored_state(checked, state)?;
            completion_expression(checked, source, ordinal as usize)?;
            Ok(())
        }
        // The current ordinary completion owns return values, not state transfer
        // arguments. Those remain behind their existing successor replay.
        _ => unsupported("ordered scalar return changed its authored destination"),
    }
}

/// Select only the authored completion expression; its scalar or structural
/// consumer separately reconstructs the declared result and value evidence.
pub(crate) fn completion_expression(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    ordinal: usize,
) -> Result<checked_trees::expression::ExpressionHandle, LoweringError> {
    use checked_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
    match checked
        .statement_table
        .statements(source.statement_nodes)
        .get(ordinal)
    {
        Some(StatementNode::Expression(expression)) => Ok(*expression),
        Some(StatementNode::Transition(transition))
            if transition.exit == TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            match checked.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(expression) => Ok(*expression),
                _ => unsupported("return transition has no value completion"),
            }
        }
        _ => unsupported("return expression absent"),
    }
}
