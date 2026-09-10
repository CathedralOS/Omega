//! Source operand coordinates, independent of arithmetic template meaning.
//!
//! An enclosing short circuit may retain either operand's computation. Apply
//! nodes therefore name their exact authored operation, just as Select nodes
//! name their conditional. Do not infer a surviving operand from expression
//! shape: a known result can still require evaluating the left operand's calls.

use super::*;
use checked_trees::expression::BinaryOperator;

/// Payload-only reconstruction cannot consume an explicit semantic transfer.
/// Resolve the operand in the exact source scope, never by a matching name in
/// another machine. Unknown non-anonymous results are not evidence of bareness.
pub(super) fn cast_requires_custody(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    cast: &checked_trees::expression::TableCastExpression,
) -> Result<bool, LoweringError> {
    if !cast.semantic_domain.is_empty() {
        return Ok(true);
    }
    let machine = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .ok_or(LoweringError::Unsupported(
            "scalar cast lost its source machine",
        ))?;
    let state = checked
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.symbol == state)
        .ok_or(LoweringError::Unsupported(
            "scalar cast lost its source state",
        ))?;
    let Some(reference) =
        validation::expression_result_type_reference(checked, machine, state, cast.value)
    else {
        return if validation::has_anonymous_numeric_results(checked, cast.value) {
            Ok(false)
        } else {
            unsupported("scalar cast has no reconstructed operand type")
        };
    };
    let (_, atoms) = crate::scalar_qualifications::type_atoms(checked, reference)?;
    Ok(!atoms.is_empty())
}

pub(super) fn anonymous_match_value(
    checked: &CheckedTrees,
    source: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Match(dispatch) = checked.expression_table.expression(source) else {
        return None;
    };
    validation::select_anonymous_numeric_match_arm(&checked.typed, dispatch, |expression| {
        checked
            .facts
            .operators
            .expression_use(expression)
            .is_none_or(|operator_use| {
                operator_use.status
                    == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
    })
}

pub(super) fn folded_match_scope(
    checked: &CheckedTrees,
    mut source: ExpressionHandle,
) -> Result<ExpressionHandle, LoweringError> {
    let mut visited = Vec::new();
    while let Some(selected) = anonymous_match_value(checked, source) {
        if visited.contains(&source) || !checked.expression_table.expression_is_valid(selected) {
            return unsupported("folded dispatch has stale or cyclic selected source");
        }
        visited.push(source);
        source = selected;
    }
    Ok(source)
}

/// Pure lowering retains the whole expression. Computation lowering can instead
/// return one operand when a known Boolean condition skips call-bearing syntax.
/// Retain its exact occurrence, without treating operand location as guard proof.
pub(super) fn value(
    checked: &CheckedTrees,
    mut source: ExpressionHandle,
    value_source: ExpressionHandle,
) -> Result<ExpressionHandle, LoweringError> {
    if !authored_expressions(checked, source)?.contains(&value_source) {
        return unsupported("pure computation escaped its authored operand scope");
    }
    while source != value_source {
        let selected = folded_match_scope(checked, source)?;
        if selected != source {
            source = selected;
            continue;
        }
        let (condition, selected, _) = selection(checked, source)?;
        source = if authored_expressions(checked, condition)?.contains(&value_source) {
            condition
        } else {
            selected
        };
    }
    Ok(source)
}

pub(super) fn application(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    mut source: ExpressionHandle,
    arity: usize,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(source) || visited.contains(&source) {
            return unsupported("computed application has a stale or cyclic source");
        }
        visited.push(source);
        match checked.expression_table.expression(source) {
            ExpressionNode::Cast(cast) if cast_requires_custody(checked, machine, state, cast)? => {
                return unsupported("computed application cannot erase a semantic qualification");
            }
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                return unsupported("computed application substituted a conditional occurrence");
            }
            ExpressionNode::Binary(binary) if arity == 2 => {
                // Comparison normalization reverses values, not evaluation.
                return Ok(vec![binary.left, binary.right]);
            }
            ExpressionNode::Unary(unary) if arity == 1 => return Ok(vec![unary.operand]),
            ExpressionNode::Cast(cast) if arity == 1 => return Ok(vec![cast.value]),
            // A same-carrier cast changes policy without adding an operation.
            ExpressionNode::Cast(cast) => source = cast.value,
            _ => return unsupported("computed application lost its authored operand positions"),
        }
    }
}

pub(super) fn selection(
    checked: &CheckedTrees,
    source: ExpressionHandle,
) -> Result<(ExpressionHandle, ExpressionHandle, bool), LoweringError> {
    if checked
        .facts
        .operators
        .expression_use(source)
        .is_some_and(|operator_use| {
            operator_use.status != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        })
    {
        return unsupported("computed selection is not an authored builtin conditional");
    }
    if let ExpressionNode::Binary(binary) = checked.expression_table.expression(source) {
        match binary.operator {
            BinaryOperator::And => return Ok((binary.left, binary.right, true)),
            BinaryOperator::Or => return Ok((binary.left, binary.right, false)),
            _ => {}
        }
    }
    unsupported("computed selection lost its authored conditional operands")
}
