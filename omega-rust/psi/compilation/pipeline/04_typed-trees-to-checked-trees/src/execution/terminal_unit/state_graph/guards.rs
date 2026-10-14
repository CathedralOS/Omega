//! Retained guard expressions and source-checked arrival contracts.
//!
//! Checking proves every state transfer, including the implicit requirements
//! from qualified parameters and their fields. Planning consumes that decision;
//! it does not re-prove membership by recognizing carrier shapes. Exact operand
//! transfer, ownership, and emitted qualification transport remain separate.

use super::super::{
    CheckFacts, CheckedComposedUnitControlStatePlan, CheckedScalarExpressionRole, PrimitiveType,
};

/// Every live edge into a qualified state must have a completed call check.
/// Invocation-only entry is handled by its caller; a reentered entry still
/// checks all graph arrivals here. The decision summarizes all calls at its
/// coordinate, not a unique operand: transfer planning retains that identity.
///
/// A decision may retain a constant obligation. This permits planning only;
/// machine lowering refuses pending obligations before Terminal production.
pub(in crate::execution::terminal_unit) fn checked_qualified_arrivals(
    facts: &CheckFacts,
    planned: &[CheckedComposedUnitControlStatePlan],
    live: &[bool],
    target_index: usize,
) -> bool {
    let target = planned[target_index].state;
    let mut has_arrival = false;
    for (source, live) in planned.iter().zip(live) {
        if !live {
            continue;
        }
        for successor in source.successors() {
            if successor.target_state != target {
                continue;
            }
            has_arrival = true;
            if !facts.call_requires_decided(
                source.state,
                successor.statement_ordinal,
                successor.target_state,
            ) {
                return false;
            }
        }
    }
    has_arrival
}

/// The exact Boolean value one authored guard retains at its `Guard`
/// coordinate: the pure expression, or the unique computation root this
/// machine owns there (a selected comparison, a call, or any other checked
/// computation). The computation producer records a root only where no pure
/// expression exists, so the coordinate names at most one of the two forms.
pub(in crate::execution::terminal_unit) fn retained_guard(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
) -> Option<checked_trees::CheckedCallScalarArgument> {
    let role = CheckedScalarExpressionRole::Guard;
    if let Some(expression) =
        facts
            .values
            .scalar_expressions
            .expression_at(state, statement_ordinal, role)
    {
        return Some(checked_trees::CheckedCallScalarArgument::Pure(
            expression.clone(),
        ));
    }
    let computations = &facts.values.scalar_computations;
    let root = computations.root_at(state, statement_ordinal, role)?;
    (root.machine == machine && computations.nodes.is_valid(root.root)).then_some(
        checked_trees::CheckedCallScalarArgument::Computation(root.root),
    )
}

pub(in crate::execution::terminal_unit) fn guard_is_boolean(
    facts: &CheckFacts,
    guard: &checked_trees::CheckedCallScalarArgument,
) -> bool {
    match guard {
        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
            expression.primitive_type() == Some(PrimitiveType::Bool)
        }
        checked_trees::CheckedCallScalarArgument::Computation(root) => {
            facts
                .values
                .scalar_computations
                .nodes
                .get(*root)
                .primitive_type
                == PrimitiveType::Bool
        }
    }
}
