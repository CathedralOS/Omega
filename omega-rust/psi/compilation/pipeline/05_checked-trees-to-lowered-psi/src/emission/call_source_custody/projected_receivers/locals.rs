//! Local receivers retain the exact producer's storage and declared access.
use super::{CheckedTrees, SymbolHandle};
use checked_trees::types::TypeReferenceNode;

/// An immutable retained borrowed carrier. Shared references already name a
/// published referent; mutable slice locals name their published descriptor.
/// Nominal mutable aliases still rejoin their original backing through alias
/// replay, so retaining a descriptor never changes nominal alias semantics.
pub(super) fn retained_borrowed_carrier<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: usize,
    symbol: SymbolHandle,
) -> Option<&'a checked_trees::statement::TableLocalData> {
    let (_, local) = checked
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement)?;
    if local.is_mutable
        || !checked
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    match checked
        .type_reference_table
        .type_reference(local.type_reference)
    {
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            ..
        } => Some(local),
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Mutable,
            referee,
            ..
        } if matches!(
            checked.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Slice { .. }
        ) =>
        {
            Some(local)
        }
        _ => None,
    }
}

pub(crate) fn declaration<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: usize,
    symbol: SymbolHandle,
) -> Option<(usize, &'a checked_trees::statement::TableLocalData)> {
    let (ordinal, local) = checked
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement)?;
    // An uninitialized declaration's home is the establishment the unit
    // plan minted at its statement. Every declared structural local owns
    // its declared shape — the declaration itself establishes the root and
    // member operations fill each leaf — so the roster scans operations,
    // not initializer kinds.
    let establishment = !local.initial_value.is_valid()
        && crate::declarations::state_declaration(checked, state.symbol)
            .ok()
            .is_some_and(|(machine, _)| {
                crate::unit::attached_unit::bodies::statement_establishes_local(
                    crate::unit::attached_unit::bodies::UnitPlans::published(
                        &checked.facts.flow.terminal_unit_effects,
                    ),
                    machine.symbol,
                    state.symbol,
                    ordinal,
                    symbol,
                )
            });
    // Primitive defaults use a scalar initializer, not a structural-value
    // root. Rejoin that declaration and independently verify its exact zero
    // before treating the name as owned storage rather than an alias loan.
    let primitive_zero = !local.initial_value.is_valid()
        && checked
            .primitive_type_reference(local.type_reference)
            .and_then(|primitive| {
                crate::emission::scalar_types::terminal_scalar_type(primitive).ok()
            })
            .is_some_and(|_| {
                checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(
                        state.symbol,
                        ordinal as u32,
                        checked_trees::CheckedScalarExpressionRole::StorageInitializer,
                    )
                    .is_some_and(|(binding, _)| binding.destination == local.symbol)
            });
    // Root identity is independent of its contents. A record holding a loan
    // still owns its carrier storage; recursively classifying its fields here
    // would confuse that storage with an erased receiver alias. Consumers
    // separately replay establishment, access and every retained loan.
    ((local.initial_value.is_valid() || establishment || primitive_zero)
        && validation::reference_shell(&checked.typed, local.type_reference).is_none())
    .then_some((ordinal, local))
}
