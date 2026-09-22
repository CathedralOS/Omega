//! Whole-root linear claims and edge-alias replay for composed Unit control.
use super::super::{
    CheckFacts, CheckedUnitEntryClaimPlan, Multiplicity, PermissionAccess, PermissionEventKind,
    PermissionEventSource, SymbolHandle,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn exact_claim_alias_events(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    source_state: &typed_trees::state::State,
    ordinal: u32,
    target_state: SymbolHandle,
    source_root: SymbolHandle,
    source_claim: &CheckedUnitEntryClaimPlan,
    target_claim: &CheckedUnitEntryClaimPlan,
) -> bool {
    let Some(statement_index) = usize::try_from(ordinal).ok() else {
        return false;
    };
    let source_matches = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == source_state.symbol
                && event.source
                    == PermissionEventSource::Call {
                        statement_index,
                        call_ordinal: 0,
                        target_symbol: target_state,
                    }
                && event.kind == PermissionEventKind::Transfer
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.claim_identity == source_claim.claim_identity
                && event.root == facts::PlaceRoot::Symbol(source_root)
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
        })
        .count();
    let target_matches = facts
        .flow
        .ownership
        .permissions
        .iter()
        .map(|(_, event)| event)
        .filter(|event| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == target_state
                && event.source == PermissionEventSource::StateEntry
                && event.kind == PermissionEventKind::Establish
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Linear
                && event.obligation_live
                && event.claim_identity == target_claim.claim_identity
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
        })
        .count();
    source_matches == 1 && target_matches == 1
}
