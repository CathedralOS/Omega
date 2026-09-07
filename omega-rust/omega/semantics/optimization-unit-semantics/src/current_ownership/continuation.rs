//! Ordered continuation cleanup retires one partial owner, not the whole frontier.

use super::*;

pub(super) fn validate_partial_continuation_roster(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: &OptimizationBlock,
    trivial_discards: &[PlaceId],
    residuals: &[StructuralAffineDiscard],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block: block.id,
    };
    let mut root = None;
    let mut moved = BTreeSet::new();
    for node in &block.nodes {
        let O::CallUnit {
            structural_arguments,
            ..
        } = &node.operation
        else {
            continue;
        };
        for argument in structural_arguments {
            if argument.access != StructuralAccess::Owned || argument.path.is_empty() {
                continue;
            }
            let Some(root_type) = partial_affine_root_type(function, argument.place) else {
                continue;
            };
            if root.is_some_and(|previous| previous != (argument.place, root_type))
                || !moved.insert(argument.path.clone())
            {
                return Err(mismatch());
            }
            root = Some((argument.place, root_type));
        }
    }
    let Some((root, root_type)) = root else {
        return if residuals.is_empty() {
            Ok(())
        } else {
            Err(mismatch())
        };
    };
    let expected = partial_affine_residuals(structural_types, root_type, &moved, residuals.len())
        .ok_or_else(mismatch)?;
    if !trivial_discards.is_empty()
        || expected.len() != residuals.len()
        || residuals
            .iter()
            .zip(expected)
            .any(|(actual, (path, structural_type))| {
                actual.place != root
                    || actual.path != path
                    || actual.structural_type != structural_type
            })
    {
        return Err(mismatch());
    }
    Ok(())
}

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
    if let Some(first) = residuals.first() {
        let root = first.place;
        let moved = frontier
            .partial_custody_paths
            .get(&root)
            .ok_or_else(mismatch)?;
        if !trivial_discards.is_empty()
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
                residuals,
            )
        {
            return Err(mismatch());
        }
        frontier.partial_custody_paths.remove(&root);
        frontier.owned_places.remove(&root);
    }
    // Empty complements have already retired at the projected call. Linear
    // partial custody remains live across ordinary edges under its old rules.
    if frontier
        .partial_custody_paths
        .keys()
        .any(|root| partial_affine_root_type(function, *root).is_some())
    {
        return Err(mismatch());
    }
    Ok(())
}
