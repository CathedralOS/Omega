//! Rejoin callback registrar occurrences to their target-closed import plans.

mod operations;

use backend_plan::BoundNominalCallbackPlacement;
use calling_conventions::ExternalBindingRow;
use operations::CallbackTerminalOperations;
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use semantic_vocabulary::OperationId;
use terminal_psi::TerminalModule;

/// One authored placement's retained Terminal registrar operation.
///
/// Several placements may share a registrar operation; each still settles in
/// supplied order against its own exact target-closed materialization.
#[derive(Clone, Copy)]
pub struct CallbackRegistrarOccurrence {
    pub placement_index: usize,
    pub terminal_operation: OperationId,
}

/// Rejoin target-closed callback registrar plans to their selected import
/// rows while the checked calling-policy realization and canonical Terminal
/// operation still coexist. The locator remains the target package's only
/// contribution; the registrar requirement owns its complete physical plan.
///
/// The occurrence iterator is cloned to prepare the requested operation roster;
/// the original order determines diagnostics and row updates. An absent import
/// remains valid for a check-only product, but an ambiguous import rejects.
/// This closure does not grant native publication or registration authority.
pub fn settle_callback_registrar_imports(
    terminal_module: &TerminalModule,
    callback_placements: &[BoundNominalCallbackPlacement],
    callback_occurrences: impl Iterator<Item = CallbackRegistrarOccurrence> + Clone,
    calling_plan_realizations: &[BoundaryCallingPlanRealization],
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
        terminal_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations),
    );
    let declarations = terminal_module
        .boundary_machines
        .iter()
        .map(|declaration| (declaration.id, declaration.identity.as_str()))
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

        let matching_realizations = calling_plan_realizations
            .iter()
            .filter(|realization| {
                realization
                    .materialized_signature()
                    .owner_requirement_identity()
                    == requirement
                    && realization.callback_context_closed
                    && realization.exact_boundary_entry_plan()
                        == &materialization.registrar_boundary_entry_plan
            })
            .collect::<Vec<_>>();
        let [realization] = matching_realizations.as_slice() else {
            return Err(format!(
                "callback registrar `{requirement}` rejoins {} exact target-closed calling-plan realizations",
                matching_realizations.len(),
            ));
        };
        let validated = realization.replayed_validated_plan().map_err(|error| {
            format!("callback registrar `{requirement}` target-closed plan failed replay: {error}")
        })?;
        if validated.plan() != &materialization.registrar_boundary_entry_plan {
            return Err(format!(
                "callback registrar `{requirement}` target-closed plan changed during replay",
            ));
        }

        let mut matching_rows = rows
            .iter_mut()
            .filter(|row| {
                row.requirement_identity == requirement
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
