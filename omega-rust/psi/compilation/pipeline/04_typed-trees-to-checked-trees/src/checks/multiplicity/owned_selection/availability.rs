//! Apply the all-path availability of an already recorded captured selection.
//! Its incoming receipt keeps alternatives; an unknown joined scalar claim is
//! never an alternative origin by itself.
use crate::checks::multiplicity::linear_obligations::LinearPlace;
use checked_trees::{FlowOwnedSelectionReceipt, FlowOwnershipFacts};
use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};

pub(in crate::checks::multiplicity) fn apply_availability(
    _program: &dyn validation::CheckingInput,
    ownership: &checked_trees::FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    places: &mut [LinearPlace],
) {
    let transfers = ownership
        .selection_transfers
        .span_or_empty(receipt.transfers);
    for transfer in transfers {
        let path = ownership.segments.span_or_empty(transfer.path);
        for place in places
            .iter_mut()
            .filter(|place| super::source_frontier_matches(place, transfer.root, path))
        {
            place.live = false;
        }
    }
    // An anonymous result already crossed its outgoing edge through the
    // captured sources; it leaves no joined local debt behind.
    if !receipt.destination.is_valid() {
        return;
    }
    let prefix = ownership.segments.span_or_empty(receipt.destination_path);
    for place in places
        .iter_mut()
        .filter(|place| place.root == receipt.destination && place.path.starts_with(prefix))
    {
        let relative = &place.path[prefix.len()..];
        let mut origins = Vec::new();
        for transfer in transfers {
            let destination = ownership.segments.span_or_empty(transfer.destination_path);
            let Some(suffix) = relative.strip_prefix(destination) else {
                continue;
            };
            let mut source = ownership.segments.span_or_empty(transfer.path).to_vec();
            source.extend_from_slice(suffix);
            for claim in ownership
                .selection_transfer_claims
                .span_or_empty(transfer.claims)
            {
                if ownership.segments.span_or_empty(claim.path) == source {
                    origins.push((claim.claim_identity, claim.provenance));
                }
            }
        }
        let origin = origins
            .first()
            .copied()
            .filter(|first| origins.iter().all(|origin| origin == first));
        place.live = true;
        place.case_excluded = false;
        place.ever_established = true;
        place.zero_image = false;
        place.claim_identity = Some(
            origin
                .map(|origin| origin.0)
                .unwrap_or(PermissionClaimIdentity::Unknown),
        );
        place.provenance = Some(
            origin
                .map(|origin| origin.1)
                .unwrap_or(PermissionProvenance::Unknown),
        );
        if place.multiplicity == Multiplicity::Affine {
            place.claim_identity = None;
        }
    }
}

/// A captured source now lives in the selection's continuation roster. Its
/// unavailable affine home cannot also retain an ordinary exit disposition.
/// A receiving whole write is still live and keeps its new storage occurrence;
/// exact linear descendants keep their separate claim and cleanup accounting.
pub(in crate::checks::multiplicity) fn retire_source_storage(
    ownership: &FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    places: &[LinearPlace],
    remaining: &mut crate::checks::multiplicity::cleanup_dispositions::RemainingCustody,
) {
    for source in ownership.selection_sources.span_or_empty(receipt.sources) {
        let root = facts::PlaceRoot::Symbol(source.symbol);
        if places.iter().any(|place| {
            place.root == root
                && place.path.is_empty()
                && place.multiplicity == Multiplicity::Affine
                && place.ever_established
                && !place.live
        }) {
            remaining.transfer_storage(root, &[]);
        }
    }
}
