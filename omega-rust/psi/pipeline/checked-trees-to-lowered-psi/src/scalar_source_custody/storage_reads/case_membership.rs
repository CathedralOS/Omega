//! A valid case in the emitted sum does not prove it was the authored case.
//! Rejoin the selected nominal meaning and parameter occurrence before comparing
//! the checked observation. This also detects erasure or operand substitution.

use super::*;

pub(super) fn authored(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
) -> Result<Option<(symbols::SymbolHandle, String)>, LoweringError> {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return Ok(None);
    };
    if !matches!(
        binary.operator,
        checked_trees::expression::BinaryOperator::Equal
            | checked_trees::expression::BinaryOperator::CaseMembership
    ) {
        return Ok(None);
    }
    let (machine, _) = authored_state(checked, state.symbol)?;
    if !validation::has_exact_case_membership_meaning(
        &checked.typed,
        machine,
        Some(state),
        expression,
        binary,
    ) {
        return Ok(None);
    }
    let subject = match checked.expression_table.expression(binary.left) {
        ExpressionNode::Borrow(borrow) => borrow.target,
        _ => binary.left,
    };
    let ExpressionNode::Name(subject) = checked.expression_table.expression(subject) else {
        return Ok(None);
    };
    if !checked
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == subject.symbol)
    {
        return Ok(None);
    }
    let ExpressionNode::Name(selected) = checked.expression_table.expression(binary.right) else {
        return Ok(None);
    };
    let case = checked
        .data_definitions()
        .iter()
        .flat_map(|data| checked.data_members(data))
        .find_map(|member| match member {
            checked_trees::data::DataMember::Variant(case) if case.symbol == selected.symbol => {
                Some(case)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "case observation has no selected case declaration",
        ))?;
    let identity = case
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| case.name.as_str().to_owned());
    Ok(Some((subject.symbol, identity)))
}
