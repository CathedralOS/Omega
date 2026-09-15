//! Content evidence, empty service reach and exact affine discards.

use crate::execution::terminal_unit::{
    CheckFacts, Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, StateParameter, SymbolHandle, parameter_root_symbol,
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
            .partition_compositions
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
}

pub(crate) fn service_reach_is_empty(
    facts: &CheckFacts,
    summary: language_semantics::ServiceReachSummary,
) -> bool {
    facts
        .service_reaches
        .rows
        .services(summary.direct)
        .is_empty()
        && facts
            .service_reaches
            .rows
            .services(summary.transitive)
            .is_empty()
}

pub(crate) fn service_reach_plan_is_empty(
    facts: &CheckFacts,
    plan: language_semantics::ServiceReachPlan,
) -> bool {
    let published_is_empty = match plan.interface {
        language_semantics::ServiceReachInterface::InternalInferred => true,
        language_semantics::ServiceReachInterface::PublishedCeiling(row) => {
            facts.service_reaches.rows.services(row).is_empty()
        }
    };
    published_is_empty
        && facts
            .service_reaches
            .rows
            .services(plan.checked_inferred)
            .is_empty()
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
        parameter_root_symbol(machine.symbol, parameter),
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
