//! Residual reconstruction for the partial-affine return edge: the complement
//! of one partially moved owned parameter, rebuilt from the checked type plan.
//! The machines that owe it are published by `build_checked_unit_effect_plans`
//! from the same ordinary body build as every other candidate.

use crate::execution::terminal_unit::cleanup::residuals;
use crate::execution::terminal_unit::{
    BTreeMap, CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan,
};
use typed_trees::type_identity::TypeIdentityId;

pub(crate) fn partial_affine_residuals(
    types: &BTreeMap<TypeIdentityId, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: TypeIdentityId,
    moved_paths: &[(Vec<CheckedUnitStructuralPathSegment>, TypeIdentityId)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = moved_paths
        .iter()
        .map(|(path, moved_type)| (path.as_slice(), *moved_type))
        .collect::<Vec<_>>();
    residuals::reconstruct(types, source, root_type, &borrowed, usize::MAX)
}
