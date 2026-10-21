//! Bind checked storage-join names to the actual selected incoming namespace.
//! These substitutions are consumed before Terminal publication. Storage stays
//! at its dominating home, including backing observed by surviving descriptors.

use super::{ClaimBindings, ClaimEdgeBinding};
use crate::lowering_error::{LoweringError, unsupported};
use language_semantics::PermissionClaimIdentity;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SelectedDecision {
    pub(crate) expression: ExpressionHandle,
    pub(crate) decision: usize,
    pub(crate) selected: bool,
}

pub(crate) fn reserve(
    checked: &checked_trees::CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    decision: usize,
    claims: &mut ClaimBindings<'_>,
) -> Result<Vec<PermissionClaimIdentity>, LoweringError> {
    let destinations = destinations(checked, machine, state, statement, expression, decision)?;
    for destination in &destinations {
        claims.reserve_checked_binding(*destination)?;
    }
    Ok(destinations)
}

pub(crate) fn destinations(
    checked: &checked_trees::CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
    decision: usize,
) -> Result<Vec<PermissionClaimIdentity>, LoweringError> {
    let mut destinations = Vec::new();
    for (_, binding) in checked.facts.flow.ownership.selection_place_bindings.iter() {
        if binding.machine != machine
            || binding.state != state
            || binding.statement_ordinal != statement as usize
            || binding.expression != expression
            || binding.decision != decision
            || binding.selected == binding.remaining
        {
            continue;
        }
        if destinations.contains(&binding.destination.claim_identity) {
            return unsupported("selection repeats a receiving claim name");
        }
        destinations.push(binding.destination.claim_identity);
    }
    Ok(destinations)
}

pub(crate) fn edge_bindings(
    checked: &checked_trees::CheckedTrees,
    destinations: &[PermissionClaimIdentity],
    route: &[SelectedDecision],
    edge: semantic_vocabulary::EdgeId,
    claims: &ClaimBindings<'_>,
) -> Result<Vec<ClaimEdgeBinding>, LoweringError> {
    let mut bindings = Vec::new();
    for destination in destinations {
        let mut incoming = *destination;
        let mut visited = Vec::new();
        loop {
            if visited.contains(&incoming) {
                return unsupported("selected claim arrivals form a cycle");
            }
            visited.push(incoming);
            let mut candidates = checked
                .facts
                .flow
                .ownership
                .selection_place_bindings
                .iter()
                .filter_map(|(_, binding)| {
                    if binding.selected == binding.remaining
                        || binding.destination.claim_identity != incoming
                    {
                        return None;
                    }
                    route
                        .iter()
                        .find(|decision| {
                            decision.expression == binding.expression
                                && decision.decision == binding.decision
                        })
                        .map(|decision| (binding, decision.selected))
                });
            let Some((binding, selected)) = candidates.next() else {
                break;
            };
            if candidates.next().is_some() {
                return unsupported("selected claim arrival has ambiguous control provenance");
            }
            incoming = if selected {
                binding.selected.claim_identity
            } else {
                binding.remaining.claim_identity
            };
        }
        if incoming == *destination {
            return unsupported("selection edge has no checked incoming claim alternative");
        }
        bindings.push(ClaimEdgeBinding {
            edge,
            destination: crate::terminal_identities::lookup_claim_id(
                claims.bindings(),
                *destination,
            )?,
            source: crate::terminal_identities::lookup_claim_id(claims.bindings(), incoming)?,
        });
    }
    Ok(bindings)
}
