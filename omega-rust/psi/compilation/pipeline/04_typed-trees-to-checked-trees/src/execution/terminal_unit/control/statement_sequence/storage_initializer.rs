//! The value that initializes one mutable primitive local's storage.
//!
//! Checking retains a local's `StorageInitializer` value in exactly one of two
//! forms: a bound pure expression (`let mut count: u32 = 0;`), or a scalar
//! computation rooted at that coordinate when the initializer evaluates a call
//! or an ordered selection (`let mut stored: i32 = echo(a);`, a policy-erased
//! conversion). Both establish the same initialized storage, and later reads,
//! assignments and borrows name the local's symbol, never this value. A
//! computation that exists but belongs to another machine, authored expression
//! or carrier is a custody mismatch, not permission to fall back to the pure
//! form.
use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, LocalConstructionTrace,
    SymbolHandle, TypedTrees, checked_boolean_contains_short_circuit,
};
use checked_trees::CheckedCallScalarArgument;
use typed_trees::types::PrimitiveType;

pub(super) fn value(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
    primitive_type: PrimitiveType,
    trace: &LocalConstructionTrace,
) -> Option<CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::StorageInitializer;
    let computations = &facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.symbol
                && root.statement_ordinal == statement_index
                && root.role == role
        });
    if let Some(root) = roots.next() {
        trace.phase(
            "statement sequence: local data: mutable primitive local: computation initializer",
        );
        trace.statement(Some(statement_index));
        if roots.next().is_some()
            || root.machine != machine
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).authored_root != local.initial_value
            || computations.nodes.get(root.root).primitive_type != primitive_type
            || facts
                .values
                .scalar_expressions
                .expression_at(state.symbol, statement_index, role)
                .is_some()
        {
            return None;
        }
        return Some(CheckedCallScalarArgument::Computation(root.root));
    }
    trace.phase("statement sequence: local data: mutable primitive local: storage initializer");
    trace.statement(Some(statement_index));
    let (binding, value) =
        facts
            .values
            .scalar_expressions
            .bound_expression_at(state.symbol, statement_index, role)?;
    if binding.expression != local.initial_value
        || binding.destination != local.symbol
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
        || !crate::execution::terminal_unit::structural_scalar_store::scalar_custody_is_exact(
            program,
            facts,
            state,
            binding,
            value,
            primitive_type,
        )
        || matches!(value, CheckedScalarExpression::Boolean(expression)
            if checked_boolean_contains_short_circuit(expression))
    {
        return None;
    }
    Some(CheckedCallScalarArgument::Pure(value.clone()))
}
