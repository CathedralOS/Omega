//! Ordered continuation cleanup retires one partial owner, not the whole frontier.
use crate::OptimizationUnitValidationError;
use crate::current_ownership::CurrentOwnership;
use crate::current_ownership::residuals::partial_affine_residuals;
use crate::current_ownership::structural::partial_affine_root_type;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAffineDiscard, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeDeclaration,
};

pub(crate) fn valid_partial_continuation_complement(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    moved: &BTreeSet<Vec<StructuralPathSegment>>,
    residuals: &[StructuralAffineDiscard],
) -> bool {
    let Some(root_type) = partial_affine_root_type(function, root) else {
        return false;
    };
    let Some(expected) =
        partial_affine_residuals(structural_types, root_type, moved, residuals.len())
    else {
        return false;
    };
    !moved.is_empty()
        && !residuals.is_empty()
        && residuals.len() == expected.len()
        && residuals
            .iter()
            .zip(expected)
            .all(|(actual, (path, structural_type))| {
                actual.place == root
                    && actual.path == path
                    && actual.structural_type == structural_type
            })
}

pub(super) fn apply_edge_partial_affine_discards(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    trivial_discards: &[PlaceId],
    residuals: &[StructuralAffineDiscard],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    if residuals.is_empty() {
        return Ok(());
    }
    let mut eligible =
        super::mutations::expected_trivial_affine_discards(function, frontier).into_iter();
    let mut remaining = residuals;
    while let Some(first) = remaining.first() {
        let root = first.place;
        let group_length = remaining
            .iter()
            .take_while(|discard| discard.place == root)
            .count();
        let (group, rest) = remaining.split_at(group_length);
        let moved = frontier
            .partial_custody_paths
            .get(&root)
            .ok_or_else(mismatch)?;
        if trivial_discards.contains(&root)
            || !eligible.any(|place| place == root)
            || frontier.owned_places.get(&root) != Some(&StructuralMultiplicity::Affine)
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(root))
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == root)
            || !valid_partial_continuation_complement(
                function,
                structural_types,
                root,
                moved,
                group,
            )
        {
            return Err(mismatch());
        }
        frontier.partial_custody_paths.remove(&root);
        frontier.owned_places.remove(&root);
        remaining = rest;
    }
    // A partial owner may remain live across internal edges. Only explicit
    // disposal consumes its complement; successor joins compare the complete
    // carried frontier and later uses retain the same moved-path exclusions.
    Ok(())
}
