//! Content evidence, empty service reach and exact affine discards.

use crate::execution::terminal_unit::{
    CheckFacts, Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, StateParameter, SymbolHandle,
};

pub(crate) fn machine_has_content_evidence(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> bool {
    facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        || facts
            .qualifications
            .content
            .partition_returns
            .iter()
            .any(|fact| fact.plan.owner == machine && fact.plan.callable == state)
}

pub(crate) fn has_exact_root_affine_discard(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    has_exact_symbol_affine_discard(
        facts,
        machine,
        state,
        parameter.symbol,
        language_semantics::PermissionProvenance::Unknown,
    )
}

pub(crate) fn has_exact_symbol_affine_discard(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    symbol: SymbolHandle,
    provenance: language_semantics::PermissionProvenance,
) -> bool {
    let matching = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.source == PermissionEventSource::StateExit
                && event.kind == PermissionEventKind::AffineDrop
                && event.access == PermissionAccess::Owned
                && event.multiplicity == Multiplicity::Affine
                && event.claim_identity == PermissionClaimIdentity::Unknown
                && event.provenance == provenance
                && !event.obligation_live
                && event.root == facts::PlaceRoot::Symbol(symbol)
        })
        .map(|(_, event)| event)
        .collect::<Vec<_>>();
    let [event] = matching.as_slice() else {
        return false;
    };
    facts
        .flow
        .ownership
        .segments
        .span_or_empty(event.segments)
        .is_empty()
}

/// An empty consuming body must dispose of the same whole claim that entered.
/// Type-owner selection and contextual hook admission remain separate checks;
/// this correspondence prevents a cleanup plan from silently losing linear debt.
pub(super) fn has_exact_root_nominal_disposition(
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameter: &StateParameter,
    multiplicity: Multiplicity,
    parameter_index: u32,
    entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
) -> bool {
    let claims = entry_claims
        .iter()
        .filter(|claim| claim.parameter_index == parameter_index)
        .collect::<Vec<_>>();
    if multiplicity == Multiplicity::Affine {
        return claims.is_empty()
            && has_exact_root_affine_discard(facts, machine, state, parameter);
    }
    let [claim] = claims.as_slice() else {
        return false;
    };
    if multiplicity != Multiplicity::Linear
        || !claim.path.is_empty()
        || claim.claim_identity == PermissionClaimIdentity::Unknown
    {
        return false;
    }
    let root = facts::PlaceRoot::Symbol(parameter.symbol);
    let mut matching = facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == root
                && event.source == PermissionEventSource::StateExit
        });
    let Some((_, event)) = matching.next() else {
        return false;
    };
    matching.next().is_none()
        && event.evaluation == checked_trees::FlowPermissionEvaluation::Statement
        && event.kind == PermissionEventKind::Consume
        && event.access == PermissionAccess::Owned
        && event.multiplicity == Multiplicity::Linear
        && event.claim_identity == claim.claim_identity
        && event.provenance != language_semantics::PermissionProvenance::Unknown
        && event.segments.is_empty()
        && !event.obligation_live
}
