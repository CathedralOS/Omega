//! Final descriptor dispositions belong to the selected completion edge.
//! Retain the checked dependency order after capturing return operands; the
//! ordinary and composed emitters resolve their own result namespaces here.
//! Resolve a projected disposition against its selected declaration before
//! appending it to the captured storage path; case identities require that type.

use super::{CleanupRegion, LoweringError};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::{Evaluation, StructuralValueStorage};
use checked_trees::CheckedUnitEffectOperationPlan;
use terminal_psi::{StructuralParameterDeclaration, TerminalAffineCleanupAction};

pub(crate) fn lower_reference_completion_regions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    planned_operations: &[CheckedUnitEffectOperationPlan],
    mut authored_regions: Vec<CleanupRegion>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
    mut result_storage: impl FnMut(u32) -> Result<StructuralValueStorage, LoweringError>,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    let mut carriers = Vec::new();
    // Stage 04 already orders releases by actual holder dependencies. Reversing
    // that schedule would retire backing before the descriptors borrowing it.
    for operation in planned_operations {
        let CheckedUnitEffectOperationPlan::ReleaseReference {
            source,
            path,
            case_conditions,
            select_evaluated_storage,
            defer_to_exit: true,
            ..
        } = operation
        else {
            continue;
        };
        let storage = evaluation.value_storage(source, parameters, &mut result_storage)?;
        let suffix =
            crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                identities,
                path,
                storage.selected_type,
                &evaluation.structural_types,
            )?;
        let conditions = super::lower_cleanup_conditions(
            identities,
            checked,
            &checked.facts.flow.terminal_unit_effects.cleanup_control,
            *case_conditions,
            &evaluation.structural_types,
            |identities, selected_path| {
                let mut selected = storage.path.clone();
                selected.extend(crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, selected_path, storage.selected_type, &evaluation.structural_types,
                )?);
                Ok((storage.place, selected))
            },
        )?;
        let mut path = storage.path;
        path.extend(suffix.clone());
        let place = storage.place;
        let action = if path.is_empty() {
            evaluation.descriptor_retirement(place, operations)?
        } else {
            let leaf = terminal_semantics::runtime_structural_path_tip(
                evaluation.structural_types.iter(),
                storage.selected_type,
                &suffix,
            )
            .ok_or(LoweringError::Unsupported(
                "deferred release lost its carrier path",
            ))?;
            TerminalAffineCleanupAction::DiscardResidual(terminal_psi::StructuralAffineDiscard {
                place,
                path: path.clone(),
                structural_type: leaf.id,
            })
        };
        if !carriers
            .iter()
            .any(|region: &CleanupRegion| region.action == action)
        {
            carriers.push(CleanupRegion {
                selected_region: select_evaluated_storage.then(|| {
                    terminal_psi::StructuralArgument {
                        place,
                        path,
                        access: terminal_psi::StructuralAccess::Owned,
                    }
                }),
                conditions,
                action,
            });
        }
    }
    // Authored disposition owns the same region, not a second death. Splice
    // it at the checked descriptor's dependency position before reconciling
    // production cleanup; this also keeps ordinary scalar returns from
    // automatically discarding a descriptor already disposed by this exit.
    for carrier in &mut carriers {
        if let Some(position) = authored_regions.iter().position(|region| {
            region.action.place() == carrier.action.place()
                && region.action.path() == carrier.action.path()
        }) {
            *carrier = authored_regions.remove(position);
        }
    }
    carriers.extend(authored_regions);
    Ok(carriers)
}
