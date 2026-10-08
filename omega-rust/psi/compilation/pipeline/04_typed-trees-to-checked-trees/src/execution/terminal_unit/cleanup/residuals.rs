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
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = types
        .iter()
        .map(|(identity, plan)| (identity.as_str(), plan))
        .collect();
    validation::reconstruct_with_boundaries(
        &borrowed,
        source,
        root_type,
        moved_paths,
        &[],
        max_residuals,
    )
}

/// Preserve cleanup operation boundaries inside the maximal live complement.
/// A boundary selects an intact receiver with its own hook; it is not a move.
/// Nested boundaries are legal and run parent-first, then in recursive reverse
/// declaration order. Duplicate hooks and hooks overlapping a move are not.
/// The same count-before-enumeration walk serves ordinary no-code residuals.
pub(super) fn reconstruct_with_boundaries(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
    boundaries: &[(&[CheckedUnitStructuralPathSegment], &str)],
    max_residuals: usize,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = types
        .iter()
        .map(|(identity, plan)| (identity.as_str(), plan))
        .collect();
    validation::reconstruct_with_boundaries(
        &borrowed,
        source,
        root_type,
        moved_paths,
        boundaries,
        max_residuals,
    )
}
