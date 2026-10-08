//! Type-directed maximal residual complement for finite records and arrays.

use super::{
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan, LoweringError,
};
pub(super) fn reconstruct(
    types: &[CheckedUnitStructuralTypePlan],
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(&[CheckedUnitStructuralPathSegment], &str)],
) -> Result<Vec<CheckedUnitPartialAffineDiscardPlan>, LoweringError> {
    if moved_paths.iter().any(|(path, _)| path.is_empty()) {
        return (moved_paths.len() == 1 && moved_paths[0].1 == root_type)
            .then_some(Vec::new())
            .ok_or(LoweringError::Unsupported(
                "partial affine Unit field path or type identity drifted",
            ));
    }
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
        "partial affine Unit field path or type identity drifted",
    ))
}
