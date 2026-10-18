//! The checked receiving cohort closes its anonymous selected complements at
//! the same boundary. Only descriptor leaves die here; scalar/root cleanup
//! stays at normal death. Case paths are guarded against each residual's own
//! active tag, because different predecessors may supply different cases.

use super::{LoweringError, OperationFrame};
use crate::expression_preparation::bindings::{structural_cases, structural_paths};
use crate::unit::unit_cleanup::{CleanupCaseCondition, CleanupRegion};
use checked_trees::{CheckedUnitSelectedReferenceRetirementPlan, CheckedUnitStructuralPathSegment};
use terminal_psi::{StructuralAffineDiscard, StructuralTypeShape, TerminalAffineCleanupAction};

impl OperationFrame<'_, '_> {
    pub(super) fn retire_selection_complements(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        complements: &[CheckedUnitSelectedReferenceRetirementPlan],
    ) -> Result<(), LoweringError> {
        let mut regions = Vec::new();
        let mut retired = Vec::new();
        for complement in complements {
            let selection = self
                .evaluation
                .selection_cleanups
                .iter()
                .find(|cleanup| cleanup.selection == complement.selection)
                .ok_or(LoweringError::Unsupported(
                    "selected reference retirement lost its join",
                ))?;
            for residual in &selection.sources {
                let place = residual.storage.place;
                let (root_type, _) = self.evaluation.carrier_signature(place, self.operations)?;
                let types = self.structural_types.declarations();
                let path = structural_paths::lower_static_argument_path(
                    identities,
                    &complement.path,
                    root_type,
                    types,
                )?;
                let leaf =
                    terminal_semantics::runtime_structural_path_tip(types.iter(), root_type, &path)
                        .ok_or(LoweringError::Unsupported(
                            "selected residual lost its reference leaf",
                        ))?;
                if !matches!(leaf.shape, StructuralTypeShape::Reference { .. }) {
                    return Err(LoweringError::Unsupported(
                        "selected complement retirement requires descriptor custody",
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
                retired.push((complement.selection, place, path.clone()));
                let action = if path.is_empty() {
                    // A standalone descriptor has no containing home to keep.
                    // End it once; its later conditional death is then absent.
                    self.evaluation
                        .descriptor_retirement(place, self.operations)?
                } else {
                    TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place,
                        path,
                        structural_type: leaf.id,
                    })
                };
                regions.push(CleanupRegion {
                    declaration_order: None,
                    selected_region: Some(terminal_psi::StructuralArgument {
                        place,
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::Owned,
                    }),
                    conditions,
                    action,
                });
            }
        }
        for (selection, place, path) in retired {
            let cleanup = self
                .evaluation
                .selection_cleanups
                .iter_mut()
                .find(|cleanup| cleanup.selection == selection)
                .ok_or(LoweringError::Unsupported(
                    "selected residual retirement lost its receipt",
                ))?;
            cleanup.retired_reference_paths.push((place, path));
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
