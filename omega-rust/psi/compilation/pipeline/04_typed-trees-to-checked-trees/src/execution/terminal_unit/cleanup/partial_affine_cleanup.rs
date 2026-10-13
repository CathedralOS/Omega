//! Residual reconstruction for the partial-affine return edge: the complement
//! of one partially moved owned parameter, rebuilt from the checked type plan.
//! The machines that owe it are published by `build_checked_unit_effect_plans`
//! from the same ordinary body build as every other candidate.

use crate::execution::terminal_unit::cleanup::residuals;
use crate::execution::terminal_unit::{
    BTreeMap, CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
};

pub(crate) fn partial_affine_residuals(
    types: &BTreeMap<typed_trees::type_identity::TypeIdentityId, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: typed_trees::type_identity::TypeIdentityId,
    moved_paths: &[(
        Vec<CheckedUnitStructuralPathSegment>,
        typed_trees::type_identity::TypeIdentityId,
    )],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = moved_paths
        .iter()
        .map(|(path, moved_type)| (path.as_slice(), *moved_type))
        .collect::<Vec<_>>();
    residuals::reconstruct(types, source, root_type, &borrowed, usize::MAX)
}
