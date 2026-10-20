//! Exact claim receipts for an owner-authorized recursive cleanup region.

use super::linear_obligations::LinearPlace;
use facts::{PlaceRoot, PlaceSegment};
use language_semantics::{Multiplicity, PermissionClaimIdentity, PermissionProvenance};
use typed_trees::types::TypeReferenceHandle;

/// A transparent container has no aggregate claim. Its one recursive cleanup
/// plan consumes the existing live claims below the selected region. Absence
/// comes from the retained contextual disposition's exact exclusions, checked
/// against current claim custody, never from a missing type-frontier entry.
pub(super) fn claim_receipts<'a>(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    root: PlaceRoot,
    path: &[PlaceSegment],
    places: &'a [LinearPlace],
    removed: &[&[PlaceSegment]],
    inactive_cases: &[&[PlaceSegment]],
) -> Option<Vec<&'a LinearPlace>> {
    let mut receipts = places
        .iter()
        .filter(|place| {
            place.root == root
                && place.live
                && place.multiplicity == Multiplicity::Linear
                && place.path.starts_with(path)
        })
        .collect::<Vec<_>>();
    // A nominal region's own receipt precedes receipts for contained claims.
    receipts.sort_by_key(|place| place.path != path);
    // Runtime receipts do not enumerate erased semantic obligations. Keep
    // the type-side check for those subjects even when no runtime claim lives
    // here. Ordinary affine-only cleanup retains its contextual admission.
    if receipts.is_empty() && !super::type_carries_linear_obligation(program, reference) {
        return Some(receipts);
    }
    if receipts.iter().any(|place| {
        matches!(
            place.claim_identity,
            None | Some(PermissionClaimIdentity::Unknown)
        ) || matches!(place.provenance, None | Some(PermissionProvenance::Unknown))
    }) {
        return None;
    }
    if receipts.iter().any(|place| {
        removed
            .iter()
            .chain(inactive_cases)
            .any(|removed| place.path[path.len()..].starts_with(removed))
    }) {
        return None;
    }
    crate::flow::residual_cleanup_is_eligible(program, reference, removed, inactive_cases)
        .then_some(receipts)
}
