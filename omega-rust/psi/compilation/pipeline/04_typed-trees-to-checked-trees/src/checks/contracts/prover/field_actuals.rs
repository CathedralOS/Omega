//! Exact referent substitution for supplied Boolean field facts at call entry.

use validation::CanonicalPlace;

use facts::{FactPlan, PlaceRoot};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

/// None leaves unrelated computed-value proof paths unchanged. A recognized
/// place requirement returns Some(false) on mismatch and cannot use labels.
pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &FactPlan,
    contexts: &[facts::FactContextHandle],
    caller_state: SymbolHandle,
    statement_index: usize,
    call: &validation::CallSite<'_>,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<bool> {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_)
    ) {
        return None;
    }
    // Implicit receivers retain their existing receiver-substitution owner.
    // Establish this helper's explicit-formal scope before field preflight.
    match has_explicit_formal_root(program, parameters, expression) {
        Some(true) => {}
        Some(false) => return None,
        None => return Some(false),
    }
    let Some(mut required) = checked_place(program, expression) else {
        return Some(false);
    };
    let formal = required.root.as_symbol()?;
    let position = typed_trees::signature::state_parameter_position(parameters, formal)?;
    let Some(argument) = validation::call_site_argument_expressions(program, call)
        .get(position)
        .copied()
    else {
        return Some(false);
    };
    let Some(raw_actual) = checked_place(program, argument) else {
        return Some(false);
    };
    // Constructed/call-produced values retain their existing value evaluator;
    // this proof path is only substitution of an existing storage referent.
    if !matches!(raw_actual.root, PlaceRoot::Symbol(_)) {
        return None;
    }
    let Some(mut actual) = validation::canonical_place_from_expression_in_state(
        program,
        caller_state,
        statement_index,
        argument,
    ) else {
        return Some(false);
    };
    actual.segments.append(&mut required.segments);
    Some(
        super::boolean_value_at_place(program, operators, semantic, contexts, &actual)
            == Some(true),
    )
}

fn has_explicit_formal_root(
    program: &dyn ::validation::CheckingInput,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> Option<bool> {
    let mut current = expression;
    let mut visited = Vec::new();
    while program.expression_table.expression_is_valid(current) && !visited.contains(&current) {
        visited.push(current);
        current = match program.expression_table.expression(current) {
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Name(name) => {
                if !name.head_symbol.is_valid() || !name.symbol.is_valid() {
                    return None;
                }
                return Some(parameters.iter().any(|parameter| {
                    parameter.symbol == name.head_symbol || parameter.symbol == name.symbol
                }));
            }
            _ => return Some(false),
        };
    }
    None
}

pub(super) fn checked_place(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    crate::flow::projected_leaf_place(program, expression)
}
