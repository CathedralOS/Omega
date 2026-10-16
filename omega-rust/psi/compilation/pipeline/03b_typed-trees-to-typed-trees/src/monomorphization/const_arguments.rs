//! Const tuple derivation. Authored and concrete argument admission belongs
//! to immutable checking, not this loop.

use symbols::SymbolKind;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
pub(super) use validation::expression_traversal::collect_type_expressions;
pub(super) use validation::static_arguments::{
    forwarded_type, is_runtime_value_expression, is_runtime_value_subject, spelling,
};

/// The call or member already stored as this argument's const-expression
/// payload. Field projections spelled `name.member` are not in that payload;
/// they stay on `evidence_projection` until the value-binder slot reads them.
pub(super) fn runtime_subject_expression(
    program: &dyn ::validation::CheckingInput,
    argument: &StaticMachineArgument,
) -> Option<ExpressionHandle> {
    if !is_runtime_value_expression(program, argument) {
        return None;
    }
    let TypeReferenceNode::ConstExpression(expression) = program
        .type_reference_table
        .type_reference(argument.type_reference)
    else {
        return None;
    };
    match program.expression_table.expression(*expression) {
        ExpressionNode::Call(_) | ExpressionNode::Member(_) => Some(*expression),
        _ => None,
    }
}

/// Resolve the exact value-scope subject one runtime `Value` argument
/// denotes: the caller's own parameter or the nearest `let` of the same name
/// declared before the call. Only a single-segment spelling without static
/// payload can resolve, and only an ordinary `Local`/`Parameter` symbol
/// counts — a runtime subject is never a type, machine, or evidence name.
pub(super) fn resolve_runtime_subject(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    scope_limit: usize,
    argument: &StaticMachineArgument,
) -> Option<symbols::SymbolHandle> {
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.evidence_projection.is_some()
        || argument.const_literal.is_some()
    {
        return None;
    }
    if argument.symbol.is_valid() {
        let kind = program.symbols.get(argument.symbol).kind;
        // A resolved parameter is exact: a realized generic binder forwarded
        // into a specialization names that parameter, never a same-named
        // local. A resolved local re-derives its nearest binding by scope
        // below, since the symbol alone cannot order shadowed `let`s.
        if kind == SymbolKind::Parameter {
            return Some(argument.symbol);
        }
        if kind != SymbolKind::Local {
            return None;
        }
    }
    validation::nearest_scoped_local(program, state, scope_limit, argument.path.as_ref())
}

pub(super) fn has_pending_type_arguments(
    program: &dyn ::validation::CheckingInput,
    arguments: &[Option<TypeReferenceHandle>],
) -> bool {
    if program.pending_const_range_endpoints.is_empty() {
        return false;
    }
    let mut visited = Vec::new();
    let mut expressions = Vec::new();
    for argument in arguments.iter().flatten() {
        collect_type_expressions(program, *argument, &mut visited, &mut expressions);
    }
    expressions
        .iter()
        .any(|expression| program.pending_const_range_endpoints.contains(expression))
}
