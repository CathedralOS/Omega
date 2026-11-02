//! A selected mutation leaves live storage, independently of the expression's
//! returned value. Bind that storage's arriving obligations by exact place.
//! Production allocates destination names; replay authenticates both original
//! arrivals. Neither path equates their lineage or repairs an absent owner.

use super::{SelectedEvaluation, join_places};
use crate::checks::multiplicity::cleanup_dispositions::RemainingCustody;
use crate::checks::multiplicity::linear_obligations::{ClaimIdentityAllocator, LinearPlace};
use crate::checks::ownership_occurrences::EvaluationBranch;
use checked_trees::{
    FlowOwnershipFacts, FlowPermissionEvaluation, FlowSelectionPlaceArrival,
    FlowSelectionPlaceBinding,
};
use diagnostics::Diagnostic;
use language_semantics::{PermissionClaimIdentity, PermissionEventSource, PermissionProvenance};
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub(in crate::checks::multiplicity) struct SelectionSite {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement: usize,
    pub evaluation: FlowPermissionEvaluation,
}

fn arrival(place: &LinearPlace) -> Result<FlowSelectionPlaceArrival, Diagnostic> {
    let (Some(claim_identity), Some(provenance)) = (place.claim_identity, place.provenance) else {
        return Err(Diagnostic::error(format!(
            "selected live place `{}` has no exact arriving claim",
            place.name
        )));
    };
    if claim_identity == PermissionClaimIdentity::Unknown
        || provenance == PermissionProvenance::Unknown
    {
        return Err(Diagnostic::error(
            "selected storage cannot bind an unknown arriving claim",
        ));
    }
    Ok(FlowSelectionPlaceArrival {
        claim_identity,
        provenance,
    })
}

fn alternatives(
    branch: EvaluationBranch,
    current: &LinearPlace,
    sibling: &LinearPlace,
) -> Result<(FlowSelectionPlaceArrival, FlowSelectionPlaceArrival), Diagnostic> {
    let current = arrival(current)?;
    let sibling = arrival(sibling)?;
    Ok(if branch.selected() {
        (current, sibling)
    } else {
        (sibling, current)
    })
}

pub(in crate::checks::multiplicity) fn enter_produced(
    selected: &mut SelectedEvaluation<(Vec<LinearPlace>, RemainingCustody)>,
    route: &[EvaluationBranch],
    current: &mut (Vec<LinearPlace>, RemainingCustody),
    site: SelectionSite,
    ownership: &mut FlowOwnershipFacts,
    identities: &mut ClaimIdentityAllocator,
    mut leave: impl FnMut(
        EvaluationBranch,
        &mut (Vec<LinearPlace>, RemainingCustody),
        &mut FlowOwnershipFacts,
    ) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    // Cleanup may use ownership's segment arena at each departing scope.
    // Stage join bindings until that ordered walk completes so both
    // operations have one visible mutable owner rather than aliased callbacks.
    let mut bindings = Vec::new();
    selected.enter(
        route,
        current,
        |branch, current| leave(branch, current, ownership),
        |branch, current, sibling| {
            join_places(&mut current.0, &sibling.0, |place, other| {
                let (selected, remaining) = alternatives(branch, place, other)?;
                let source = PermissionEventSource::Statement {
                    statement_index: site.statement,
                };
                let claim_identity = identities.mint(site.machine, site.state, source);
                let PermissionClaimIdentity::Established { ordinal, .. } = claim_identity else {
                    return Err(Diagnostic::error(
                        "selected storage binding has no destination name",
                    ));
                };
                let destination = FlowSelectionPlaceArrival {
                    claim_identity,
                    provenance: PermissionProvenance::Joined {
                        machine_symbol: site.machine,
                        state_symbol: site.state,
                        source,
                        ordinal,
                    },
                };
                bindings.push((
                    place.path.clone(),
                    FlowSelectionPlaceBinding {
                        machine: site.machine,
                        state: site.state,
                        statement_ordinal: site.statement,
                        evaluation: site.evaluation,
                        expression: branch.expression(),
                        decision: branch.decision(),
                        root: place.root,
                        path: Default::default(),
                        selected,
                        remaining,
                        destination,
                    },
                ));
                Ok(destination)
            })?;
            current.1.join(&sibling.1);
            Ok(())
        },
    )?;
    for (path, mut binding) in bindings {
        binding.path = ownership.segments.insert_many(path);
        ownership.selection_place_bindings.insert(binding);
    }
    Ok(())
}

pub(in crate::checks::multiplicity) fn join_recorded_places(
    ownership: &FlowOwnershipFacts,
    site: SelectionSite,
    branch: EvaluationBranch,
    current: &mut [LinearPlace],
    sibling: &[LinearPlace],
) -> Result<(), Diagnostic> {
    join_places(current, sibling, |place, other| {
        let (selected, remaining) = alternatives(branch, place, other)?;
        let mut candidates = ownership
            .selection_place_bindings
            .iter()
            .filter(|(_, binding)| {
                binding.machine == site.machine
                    && binding.state == site.state
                    && binding.statement_ordinal == site.statement
                    && binding.evaluation == site.evaluation
                    && binding.expression == branch.expression()
                    && binding.decision == branch.decision()
                    && binding.root == place.root
                    && ownership.segments.span_or_empty(binding.path) == place.path
            });
        let Some((_, binding)) = candidates.next() else {
            return Err(Diagnostic::error(format!(
                "selected live place `{}` has no recorded destination binding",
                place.name
            )));
        };
        if candidates.next().is_some()
            || binding.selected != selected
            || binding.remaining != remaining
            || !destination_matches_owner(binding)
        {
            return Err(Diagnostic::error(format!(
                "selected storage binding for `{}` disagrees with its exact arrivals",
                place.name
            )));
        }
        Ok(binding.destination)
    })
}

fn destination_matches_owner(binding: &FlowSelectionPlaceBinding) -> bool {
    if binding.selected == binding.remaining {
        return false;
    }
    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = binding.destination.claim_identity
    else {
        return false;
    };
    machine_symbol == binding.machine
        && state_symbol == binding.state
        && source
            == PermissionEventSource::Statement {
                statement_index: binding.statement_ordinal,
            }
        && binding.destination.provenance
            == PermissionProvenance::Joined {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            }
        && binding.destination.claim_identity != binding.selected.claim_identity
        && binding.destination.claim_identity != binding.remaining.claim_identity
}

/// Replay of the selected paths authenticates the arrivals. Later transfers
/// must still resolve the exact destination, not merely its statement number.
pub(in crate::checks::multiplicity) fn claim_origin(
    ownership: &FlowOwnershipFacts,
    identity: PermissionClaimIdentity,
    provenance: PermissionProvenance,
) -> bool {
    let mut candidates = ownership
        .selection_place_bindings
        .iter()
        .filter(|(_, binding)| {
            binding.selected != binding.remaining
                && binding.destination.claim_identity == identity
                && binding.destination.provenance == provenance
        });
    candidates
        .next()
        .is_some_and(|(_, binding)| destination_matches_owner(binding))
        && candidates.next().is_none()
}
