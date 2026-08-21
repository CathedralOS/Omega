//! State-relative frame-path namespace queries.
//!
//! This leaf classifies whether a relative write is caller-visible, normalizes
//! authored state roots into positional frame roots, and recognizes exact
//! symbol forwarding. It does not resolve calls or infer write frames.

use super::place_paths::{append_place_suffix, split_place_root};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::StatementNode;

pub(super) fn push_visible_frame_path(
    writes: &mut Vec<String>,
    relative: String,
    parameters: &[StateParameter],
    locals: &[String],
) -> Option<()> {
    if relative_state_path_is_visible(&relative, parameters, locals)? && !writes.contains(&relative)
    {
        writes.push(relative);
    }
    Some(())
}

pub(super) fn expression_forwards_exact_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_forwards_exact_symbol(program, *inner, symbol),
        ExpressionNode::Name(path) => path.symbol == symbol,
        _ => false,
    }
}

pub(super) fn relative_state_path_is_visible(
    relative: &str,
    parameters: &[StateParameter],
    locals: &[String],
) -> Option<bool> {
    let (root, _) = split_place_root(relative);
    if root == "self"
        || parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == root)
    {
        return Some(true);
    }
    if locals.iter().any(|local| local == root) {
        return Some(false);
    }
    None
}

pub(super) fn normalize_state_relative_path(
    program: &TypedTrees,
    state: &State,
    relative: &str,
) -> Option<Option<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        return Some(Some(append_place_suffix("self", suffix)));
    }
    if let Some(parameter_index) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == root)
    {
        return Some(Some(append_place_suffix(
            &format!("$P{parameter_index}"),
            suffix,
        )));
    }
    let is_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.name.as_str() == root)
        });
    is_local.then_some(None)
}
