//! Bind checked storage-join names to the actual selected incoming namespace.
//! These substitutions are consumed before Terminal publication. Storage stays
//! at its dominating home, including backing observed by surviving descriptors.

use super::{ClaimBindings, ClaimEdgeBinding};
use crate::checked_facts::flow_facts;
use crate::lowering_error::{LoweringError, unsupported};
use crate::terminal_identities::lookup_claim_id;
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
    // Stage 05 mints every binding's destination fresh from the one claim
    // allocator that is reset with this arena, so destinations never repeat.
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
            // A destination names exactly one binding, so at most one
            // binding on this route receives `incoming`.
            let arrival = flow_facts(checked)
                .ownership
                .selection_place_bindings
                .iter()
                .find_map(|(_, binding)| {
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
            let Some((binding, selected)) = arrival else {
                break;
            };
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
            destination: lookup_claim_id(claims.bindings(), *destination)?,
            source: lookup_claim_id(claims.bindings(), incoming)?,
        });
    }
    Ok(bindings)
}
