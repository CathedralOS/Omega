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

pub(super) fn reconstruct(
    types: &BTreeMap<typed_trees::type_identity::TypeIdentityId, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: typed_trees::type_identity::TypeIdentityId,
    moved_paths: &[(
        &[CheckedUnitStructuralPathSegment],
        typed_trees::type_identity::TypeIdentityId,
    )],
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
