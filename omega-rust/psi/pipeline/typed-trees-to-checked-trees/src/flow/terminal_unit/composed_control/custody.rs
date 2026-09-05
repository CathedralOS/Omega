//! Whole-root linear claims and edge-alias replay for composed Unit control.

use super::*;

pub(super) fn exact_claims(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
) -> Option<Vec<CheckedUnitEntryClaimPlan>> {
    entry_claims(
        program,
        facts,
        machine.symbol,
        state.symbol,
        structural_parameters,
        program.state_parameters(state),
    )
}

pub(super) fn exact_structural_custody(
    entry_parameters: &[CheckedUnitStructuralParameterPlan],
    true_parameters: &[CheckedUnitStructuralParameterPlan],
    false_parameters: &[CheckedUnitStructuralParameterPlan],
    entry_claims: &[CheckedUnitEntryClaimPlan],
    true_claims: &[CheckedUnitEntryClaimPlan],
    false_claims: &[CheckedUnitEntryClaimPlan],
) -> bool {
    if entry_parameters.is_empty() && true_parameters.is_empty() && false_parameters.is_empty() {
        return entry_claims.is_empty() && true_claims.is_empty() && false_claims.is_empty();
    }
    let ([entry], [when_true], [when_false], [entry_claim], [true_claim], [false_claim]) = (
        entry_parameters,
        true_parameters,
        false_parameters,
        entry_claims,
        true_claims,
        false_claims,
    ) else {
        return false;
    };
    [entry, when_true, when_false].iter().all(|parameter| {
        !parameter.is_self
            && parameter.multiplicity == Multiplicity::Linear
            && parameter.access == CheckedStructuralAccess::Owned
            && parameter.qualifications.is_empty()
            && parameter.type_identity == entry.type_identity
    }) && [entry_claim, true_claim, false_claim].iter().all(|claim| {
        claim.parameter_index == 0
            && claim.path.is_empty()
            && claim.carry == CarryPolicy::STRICT
            && claim.claim_identity != PermissionClaimIdentity::Unknown
    })
}

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
