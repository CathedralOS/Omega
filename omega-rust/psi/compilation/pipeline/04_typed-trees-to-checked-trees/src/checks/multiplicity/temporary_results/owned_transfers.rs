//! A by-value call receives an anonymous owner whole. Its producer has already
//! established the owner's claim frontier at this statement; the consuming
//! call transfers each live claim exactly as it would transfer a named local's,
//! so the callee's outcome can continue it and call planning can rejoin it to
//! the argument that carries it.
use super::owners::{TemporaryConsumption, TemporaryOwner};
use super::{
    CheckFacts, FlowPermissionEventFact, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionProvenance,
};
use language_semantics::PermissionEventSource;

#[allow(clippy::too_many_arguments)]
pub(in crate::checks::multiplicity) fn append_owned_transfer(
    facts: &mut CheckFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[TemporaryOwner],
    places: &mut [crate::checks::multiplicity::linear_obligations::LinearPlace],
    permissions: &mut Vec<FlowPermissionEventFact>,
    conditional_carrier_transfers: &mut Vec<usize>,
) {
    for owner in owners.iter().filter(|owner| {
        matches!(owner.consumption, TemporaryConsumption::OwnedTransfer)
            && matches!(owner.consumer, PermissionEventSource::Call { statement_index: index, .. } if index == statement_index)
            && owner.evaluation == evaluation
    }) {
        let root = facts::PlaceRoot::Expression(owner.expression);
        for place in places
            .iter_mut()
            .filter(|place| place.root == root && place.live)
        {
            super::super::linear_validation::record_conditional_carrier_transfer(
                place,
                &[],
                permissions.len(),
                conditional_carrier_transfers,
            );
            permissions.push(FlowPermissionEventFact {
                expression: owner.expression,
                evaluation,
                machine_symbol: machine,
                state_symbol: state,
                source: owner.consumer,
                kind: PermissionEventKind::Transfer,
                multiplicity: place.multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: place
                    .claim_identity
                    .unwrap_or(PermissionClaimIdentity::Unknown),
                provenance: place.provenance.unwrap_or(PermissionProvenance::Unknown),
                root,
                segments: facts
                    .flow
                    .ownership
                    .segments
                    .insert_many(place.path.iter().copied()),
                obligation_live: place.multiplicity == Multiplicity::Linear,
            });
            place.live = false;
        }
    }
}
