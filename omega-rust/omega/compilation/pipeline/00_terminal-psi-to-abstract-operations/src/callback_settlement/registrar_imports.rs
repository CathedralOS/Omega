//! Rejoin callback registrar occurrences to their target-closed import plans.

use super::CallbackTerminalOperations;
use backend_plan::NativeCallbackPlacement;
use calling_conventions::BoundaryCallingApplication;
use calling_conventions::ExternalBindingRow;
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalModule;

/// One authored placement's retained Terminal registrar operation.
///
/// Several placements may share a registrar operation; each still settles in
/// supplied order against its own exact target-closed materialization.
#[derive(Clone, Copy)]
pub(crate) struct CallbackRegistrarOccurrence {
    pub placement_index: usize,
    pub terminal_operation: OperationId,
}

/// Rejoin target-closed callback registrar plans to their selected import
/// rows using the produced calling application and canonical Terminal
/// operation still coexist. The locator remains the target package's only
/// contribution; the registrar requirement owns its complete physical plan.
///
/// A Terminal boundary names the call's own requirement identity. A closed
/// application of a requirement template rejoins the template-keyed calling
/// plan and import row through the selected closure's requirement relation;
/// an unspecialized requirement selects itself.
///
/// The occurrence iterator is cloned to prepare the requested operation roster;
/// the original order determines diagnostics and row updates. An absent import
/// remains valid for a check-only product, but an ambiguous import rejects.
/// This closure does not grant native publication or registration authority.
pub(crate) fn settle_callback_registrar_imports(
    terminal_module: &TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    callback_placements: &[NativeCallbackPlacement],
    callback_occurrences: impl Iterator<Item = CallbackRegistrarOccurrence> + Clone,
    calling_applications: &[BoundaryCallingApplication],
    external_binding_rows: &[ExternalBindingRow],
) -> Result<Vec<ExternalBindingRow>, String> {
    let mut rows = external_binding_rows.to_vec();
    if callback_occurrences.clone().next().is_none() {
        return Ok(rows);
    }
    let operations = CallbackTerminalOperations::new(
        callback_occurrences
            .clone()
            .map(|occurrence| occurrence.terminal_operation),
        terminal_module.operations(),
    );
    let declarations = terminal_module
        .boundary_machines
        .iter()
        .map(|declaration| {
            (
                declaration.id,
                terminal_module
                    .identities
                    .text(declaration.identity)
                    .unwrap_or_default(),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    for occurrence in callback_occurrences {
        let placement = callback_placements
            .get(occurrence.placement_index)
            .ok_or_else(|| {
                "callback registrar import closure names an absent retained placement".to_owned()
            })?;
        let materialization = placement.private_materialization.as_ref().ok_or_else(|| {
            "callback registrar import closure requires one private materialization".to_owned()
        })?;
        let operation = operations.find(occurrence.terminal_operation).map_err(|count| {
            format!(
                "callback registrar import closure resolves Terminal operation {} to {count} operations",
                occurrence.terminal_operation.get(),
            )
        })?;
        let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind else {
            return Err(
                "callback registrar import closure names a non-boundary Terminal operation"
                    .to_owned(),
            );
        };
        let requirement = declarations.get(boundary).copied().ok_or_else(|| {
            "callback registrar import closure names an absent Terminal boundary".to_owned()
        })?;
        // The call retains its exact closed requirement. Calling applications
        // and evaluated import locators belong to the selected authored row.
        // Only the retained plan-scoped correspondence supplies that address;
        // callback materialization still supplies the complete exact plan.
        let selected_requirement = selected.selected_requirement_identity(requirement);

        let matching_realizations = calling_applications
            .iter()
            .filter(|realization| {
                realization.requirement_identity == selected_requirement
                    && realization.callback_context_closed
                    && realization.plan.plan() == &materialization.registrar_boundary_entry_plan
            })
            .collect::<Vec<_>>();
        let [_] = matching_realizations.as_slice() else {
            return Err(format!(
                "callback registrar `{requirement}` rejoins {} exact target-closed calling-plan realizations",
                matching_realizations.len(),
            ));
        };
        let mut matching_rows = rows
            .iter_mut()
            .filter(|row| {
                row.requirement_identity == selected_requirement
                    && matches!(
                        row.binding,
                        calling_conventions::ExternalBindingKind::Import { .. }
                    )
            })
            .collect::<Vec<_>>();
        if matching_rows.is_empty() {
            // Check-only Terminal custody does not require a target package to
            // supply a normalized import. Native source-import re-entry will
            // independently require and settle the exact row.
            continue;
        }
        let [row] = matching_rows.as_mut_slice() else {
            return Err(format!(
                "callback registrar `{requirement}` rejoins {} retained external import rows",
                matching_rows.len(),
            ));
        };
        row.boundary_entry_plan = Some(materialization.registrar_boundary_entry_plan.clone());
    }

    Ok(rows)
}
