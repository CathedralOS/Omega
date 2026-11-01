//! The checked receiving cohort closes its anonymous selected complements at
//! the same boundary. Only descriptor leaves die here; scalar/root cleanup
//! stays at normal death. Case paths are guarded against each residual's own
//! active tag, because different predecessors may supply different cases.

use super::{LoweringError, OperationFrame};
use crate::expression_preparation::bindings::{structural_cases, structural_paths};
use crate::unit::unit_cleanup::{CleanupCaseCondition, CleanupRegion};
use checked_trees::{CheckedUnitSelectedReferenceRetirementPlan, CheckedUnitStructuralPathSegment};
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralTypeShape, TerminalAffineCleanupAction,
};

impl OperationFrame<'_, '_> {
    pub(super) fn retire_selection_complements(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        complements: &[CheckedUnitSelectedReferenceRetirementPlan],
    ) -> Result<(), LoweringError> {
        let mut regions = Vec::new();
        let mut retired = Vec::<(
            usize,
            semantic_vocabulary::PlaceId,
            Vec<terminal_psi::StructuralPathSegment>,
        )>::new();
        for complement in complements {
            let (continuation, selection) = self
                .evaluation
                .selection_cleanups
                .iter()
                .enumerate()
                .find(|(_, cleanup)| cleanup.selection == complement.selection)
                .ok_or(LoweringError::Unsupported(
                    "selected reference retirement lost its join",
                ))?;
            // The owning production captured one complete source roster.
            // Private value joins do not contribute competing correspondences.
            for residual in &selection.sources {
                let place = residual.storage.place;
                let (root_type, _) = self.evaluation.emitted_carrier(place, self.operations)?;
                let types = self.structural_types.declarations();
                let captured = terminal_semantics::runtime_structural_path_tip(
                    types.iter(),
                    root_type,
                    &residual.storage.path,
                )
                .ok_or(LoweringError::Unsupported(
                    "selected residual lost its captured source projection",
                ))?;
                if captured.id != residual.storage.selected_type {
                    return Err(LoweringError::Unsupported(
                        "selected residual captured source has a different carrier",
                    ));
                }
                let mut path = residual.storage.path.clone();
                path.extend(structural_paths::lower_static_argument_path(
                    identities,
                    &complement.path,
                    residual.storage.selected_type,
                    types,
                )?);
                let leaf =
                    terminal_semantics::runtime_structural_path_tip(types.iter(), root_type, &path)
                        .ok_or(LoweringError::Unsupported(
                            "selected residual lost its reference leaf",
                        ))?;
                if !matches!(leaf.shape, StructuralTypeShape::Reference { .. }) {
                    return Err(LoweringError::Unsupported(
                        "selected complement retirement requires shared descriptor custody",
                    ));
                }
                let mut conditions = Vec::new();
                for (position, segment) in complement.path.iter().enumerate() {
                    let CheckedUnitStructuralPathSegment::Case(identity) = segment else {
                        continue;
                    };
                    let (selected_prefix, case) = structural_cases::resolve_at(
                        identities,
                        types,
                        residual.storage.selected_type,
                        &complement.path[..position],
                        identity,
                    )?;
                    let mut prefix = residual.storage.path.clone();
                    prefix.extend(selected_prefix);
                    let sum = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        root_type,
                        &prefix,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "selected residual case lost its carrier",
                    ))?;
                    conditions.push(CleanupCaseCondition {
                        place,
                        path: prefix,
                        structural_type: sum.id,
                        case,
                    });
                }
                // Repeated requests still validate the exact typed path, but
                // one physical descriptor has only one death at this cut.
                let duplicate = retired
                    .iter()
                    .any(|(prior_continuation, prior, prior_path)| {
                        self.evaluation.selection_cleanups[*prior_continuation].selection
                            == complement.selection
                            && *prior == place
                            && prior_path == &path
                    });
                retired.push((continuation, place, path.clone()));
                if duplicate {
                    continue;
                }
                let action = if path.is_empty() {
                    self.evaluation
                        .descriptor_retirement(place, self.operations)?
                } else {
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place,
                        path: path.clone(),
                        structural_type: leaf.id,
                    })
                };
                regions.push(CleanupRegion {
                    declaration_order: None,
                    selected_region: Some(terminal_psi::StructuralArgument {
                        place,
                        path: path.clone(),
                        access: StructuralAccess::Owned,
                    }),
                    conditions,
                    action,
                });
            }
        }
        for (continuation, place, path) in retired {
            let cleanup = &mut self.evaluation.selection_cleanups[continuation];
            if !cleanup
                .retired_reference_paths
                .iter()
                .any(|(prior, prior_path)| *prior == place && *prior_path == path)
            {
                cleanup.retired_reference_paths.push((place, path));
            }
        }
        if !regions.is_empty() {
            self.evaluation.continue_cleanup_regions(
                identities,
                self.checked,
                regions,
                self.values,
                self.next_value,
                self.next_block,
                self.next_edge,
                self.operations,
            )?;
        }
        Ok(())
    }
}
