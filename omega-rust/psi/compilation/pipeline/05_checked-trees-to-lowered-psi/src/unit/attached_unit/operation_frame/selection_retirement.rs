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
            let selections = self
                .evaluation
                .selection_cleanups
                .iter()
                .enumerate()
                .filter(|(_, cleanup)| cleanup.selection == complement.selection)
                .collect::<Vec<_>>();
            if selections.is_empty() {
                return Err(LoweringError::Unsupported(
                    "selected reference retirement lost its join",
                ));
            }
            // Nested completed values can retain several continuations of
            // this exact receipt. Keep their operation-local mappings intact;
            // an empty row cannot hide another row's concrete residual.
            for (continuation, selection) in selections {
                for residual in &selection.sources {
                    let place = residual.storage.place;
                    let (root_type, _) = self.evaluation.emitted_carrier(place, self.operations)?;
                    let types = self.structural_types.declarations();
                    let path = structural_paths::lower_static_argument_path(
                        identities,
                        &complement.path,
                        root_type,
                        types,
                    )?;
                    let leaf = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        root_type,
                        &path,
                    )
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
                        let (prefix, case) = structural_cases::resolve_at(
                            identities,
                            types,
                            root_type,
                            &complement.path[..position],
                            identity,
                        )?;
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
                    // Resolve and check each row's guarded typed path before
                    // coalescing the same physical descriptor death. Every row
                    // still records that retirement for its later normal cleanup.
                    let duplicate =
                        retired
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
