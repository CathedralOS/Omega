//! The maximal residual complement a projected owned move leaves in its root:
//! every untouched subtree of the root's finite records and arrays, in
//! reverse declaration order at each record level. Checking's partial-affine
//! complement walk (`validation::reconstruct_with_boundaries`) derives it; a
//! root with no partial-affine decomposition has no residual discard route.

use crate::LoweringError;
use checked_trees::{
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
};

/// The residual discards `moved_paths` leave in a root of type `root_type`.
pub(super) fn complement(
    types: &[CheckedUnitStructuralTypePlan],
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
) -> Result<Vec<CheckedUnitPartialAffineDiscardPlan>, LoweringError> {
    let borrowed = types
        .iter()
        .map(|declaration| (declaration.identity.as_str(), declaration))
        .collect();
    validation::reconstruct_with_boundaries(
        &borrowed,
        source,
        root_type,
        moved_paths,
        &[],
        usize::MAX,
    )
    .ok_or(LoweringError::Unsupported(
        "projected move's residual complement has no partial-affine discard route",
    ))
}
