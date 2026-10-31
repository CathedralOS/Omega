//! Type-directed maximal residual complement for finite structural transfers.
//!
//! Only the complement is validated: a partially touched node must be a
//! record, array, or exactly selected case whose fields are partial-affine
//! leaves; each untouched subtree must be a finite tree of them. Closed alternatives
//! remain whole when every alternative is custody-free; this does not select
//! a case or authorize disposal of resource-bearing case payloads. A touched
//! case instead retains only its selected payload siblings and common fields.
//! Membership and nominal-hook admission remain the caller's responsibility.
//! A moved subtree transfers whole with its own custody, so its shape (references included) is the
//! consumer's concern, not this discard roster's.
use super::{
    BTreeMap, CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
};
use typed_trees::type_identity::TypeIdentityId;

pub(super) fn reconstruct(
    types: &BTreeMap<TypeIdentityId, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: TypeIdentityId,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], TypeIdentityId)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = types
        .iter()
        .map(|(identity, plan)| (*identity, plan))
        .collect();
    validation::reconstruct(
        &borrowed,
        source,
        root_type,
        moved_paths,
        &[],
        max_residuals,
    )
}
