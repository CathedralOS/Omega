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

/// Apply the selected edge's stream without grouping whole owners ahead of
/// partial owners or hooks. A residual group is one exact owner's complement;
/// outgoing ownership bindings are installed only after this transaction.
pub(super) fn apply_edge_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<semantic_vocabulary::MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    use terminal_psi::TerminalAffineCleanupAction as Action;
    // Preserve the existing no-code subsequence ordering checks before any
    // mutation. Dispatching one action at a time must not accidentally admit
    // a reordered or duplicated root schedule.
    let eligible = super::mutations::expected_trivial_affine_discards(function, frontier);
    let mut whole_order = eligible.iter();
    let mut residual_order = eligible.iter();
    let mut previous_residual = None;
    for action in actions {
        let ordered = match action {
            Action::DiscardRoot(place) => whole_order.any(|candidate| candidate == place),
            Action::DiscardResidual(discard) if previous_residual == Some(discard.place) => true,
            Action::DiscardResidual(discard) => {
                residual_order.any(|candidate| *candidate == discard.place)
            }
            Action::InvokeNominal(_) => true,
        };
        previous_residual = match action {
            Action::DiscardResidual(discard) => Some(discard.place),
            _ => None,
        };
        if !ordered {
            return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
                machine: function.machine,
                block,
            });
        }
    }
    let mut remaining = actions;
    while let Some(action) = remaining.first() {
        match action {
            Action::DiscardRoot(place) => {
                super::mutations::apply_edge_trivial_affine_discards(
                    function,
                    structural_types,
                    block,
                    frontier,
                    std::slice::from_ref(place),
                )?;
                remaining = &remaining[1..];
            }
            Action::DiscardResidual(first) => {
                let residuals = remaining
                    .iter()
                    .map_while(|action| match action {
                        Action::DiscardResidual(discard) if discard.place == first.place => {
                            Some(discard.clone())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                apply_edge_partial_affine_discards(
                    function,
                    structural_types,
                    block,
                    frontier,
                    &[],
                    &residuals,
                )?;
                remaining = &remaining[residuals.len()..];
            }
            Action::InvokeNominal(cleanup) => {
                super::cleanup::consume_nominal_cleanup(
                    function,
                    functions,
                    structural_types,
                    block,
                    frontier,
                    cleanup,
                )?;
                remaining = &remaining[1..];
            }
        }
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
        for discard in group {
            super::references::discard_subtree(
                function,
                structural_types,
                block,
                &mut frontier.live_references,
                root,
                &discard.path,
            )?;
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
