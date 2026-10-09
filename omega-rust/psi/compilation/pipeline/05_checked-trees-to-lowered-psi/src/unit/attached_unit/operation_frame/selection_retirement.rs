//! The checked receiving cohort closes its anonymous selected complements at
//! the same boundary. Only descriptor leaves die here; scalar/root cleanup
//! stays at normal death. Case paths are guarded against each residual's own
//! active tag, because different predecessors may supply different cases.

use super::{LoweringError, OperationFrame, StructuralAccess};
use crate::expression_preparation::bindings::{structural_cases, structural_paths};
use crate::unit::unit_cleanup::{CleanupCaseCondition, CleanupRegion};
use checked_trees::{CheckedUnitSelectedReferenceRetirementPlan, CheckedUnitStructuralPathSegment};
use terminal_psi::{StructuralAffineDiscard, StructuralTypeShape, TerminalAffineCleanupAction};

impl OperationFrame<'_, '_> {
    pub(super) fn retire_selection_complements(
        &mut self,
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
            for (_, residual) in &selection.remaining {
                let place = self.evaluation.current_structural_place(*residual);
                let root_type = self
                    .evaluation
                    .structural_root_parameters()
                    .find(|declaration| declaration.place == place)
                    .map(|declaration| declaration.structural_type)
                    .ok_or(LoweringError::Unsupported(
                        "selected residual lost its receiving declaration",
                    ))?;
                let types = self.structural_types.declarations();
                let path = structural_paths::lower_static_argument_path(
                    &complement.path,
                    root_type,
                    types,
                )?;
                let leaf =
                    terminal_semantics::runtime_structural_path_tip(types.iter(), root_type, &path)
                        .ok_or(LoweringError::Unsupported(
                            "selected residual lost its reference leaf",
                        ))?;
                if !matches!(
                    leaf.shape,
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    }
                ) {
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
                retired.push((complement.selection, *residual, path.clone()));
                regions.push(CleanupRegion {
                    conditions,
                    action: TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                        place,
                        path,
                        structural_type: leaf.id,
                    }),
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
