//! Project selected declaration reach once, then settle without source trees.
//!
//! Provider derivation already retained each exact requirement and realization.
//! Reopening provider spellings here would lose inherited/captured/target-selected
//! correspondence. All selected rows participate, including dormant declarations;
//! executable Terminal reach is not the inventory for this obligation.

use crate::{ProviderSchemaDeclaration, SelectedProviderReviewProvenance};
use checked_trees::{CheckFacts, typed_trees::TypedTrees};
use diagnostics::Diagnostic;
use provider_selection::InstallationReachRow;

pub(super) fn derive_selected_installation_reach_resolutions(
    typed: &TypedTrees,
    facts: &CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<
    (
        Vec<effects::InstallationReachResolution>,
        terminal_psi::TerminalIdentityTable,
    ),
    Vec<Diagnostic>,
> {
    if selected.plans().len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "installation reach lost selected provider custody",
        )]);
    }
    let mut rows = Vec::new();
    let mut diagnostics = Vec::new();
    for (plan, retained) in selected.plans().iter().zip(provenance) {
        let provider = &retained.provider;
        if plan != &retained.plan
            || plan.rows.len() != provider.row_requirements.len()
            || plan.rows.len() != provider.row_realizations.len()
        {
            diagnostics.push(Diagnostic::error(
                "installation reach lost exact selected row custody",
            ));
            continue;
        }
        // These schema classes publish concrete reach, not bounded trait rows.
        if matches!(
            provider.schema,
            ProviderSchemaDeclaration::BoundaryOperator(_)
                | ProviderSchemaDeclaration::SealedPlacedAccessor(_)
        ) {
            continue;
        }
        for ((row, requirement), realization) in plan
            .rows
            .iter()
            .zip(&provider.row_requirements)
            .zip(&provider.row_realizations)
        {
            let projected = (|| {
                let (identity, bounded, bound) = requirement_reach(typed, *requirement)
                    .ok_or_else(|| {
                        Diagnostic::error("installation reach has no exact declared requirement")
                    })?;
                if identity != plan.identities.text(row.requirement_identity) {
                    return Err(Diagnostic::error(
                        "installation reach changed its selected requirement identity",
                    ));
                }
                if !bounded {
                    return Ok(None);
                }
                let realization = realization.authored()
                    .and_then(|symbol| typed.machine_by_symbol(symbol))
                    .ok_or_else(|| Diagnostic::error(format!(
                        "selected provider row `{identity}` has no authored realization for its installation-bound reach"
                    )))?;
                let envelope = facts
                    .contract_plans
                    .realized_envelope(realization.symbol)
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "selected provider realization `{}` has no checked contract envelope",
                            realization.name
                        ))
                    })?;
                let dependencies = envelope.unresolved_installation_reaches.iter()
                    .map(|dependency| requirement_reach(typed, dependency.requirement)
                        .map(|(identity, _, _)| identity)
                        .ok_or_else(|| Diagnostic::error(format!(
                            "selected provider realization `{}` lost an exact installation-bound requirement", realization.name
                        ))))
                    .collect::<Result<Vec<_>, _>>()?;
                let reach = if dependencies.is_empty() {
                    envelope.effective_service_reach.clone()
                } else {
                    envelope.concrete_service_reach.clone()
                };
                Ok(Some(InstallationReachRow {
                    requirement_identity: identity,
                    provider_plan_report_identity: plan.report_fingerprint(),
                    provider_name: plan.provider_type.clone(),
                    realization_name: realization.name.as_str().to_owned(),
                    upper_bound: facts
                        .service_reaches
                        .rows
                        .services(bound)
                        .iter()
                        .filter_map(|service| facts.service_reaches.services.definition(*service))
                        .map(|definition| definition.name.clone())
                        .collect(),
                    concrete_reach: reach,
                    dependencies,
                }))
            })();
            match projected {
                Ok(Some(row)) => rows.push(row),
                Ok(None) => {}
                Err(error) => diagnostics.push(error),
            }
        }
    }
    let settled = provider_selection::resolve_installation_reach(rows);
    match settled {
        Ok(settled) if diagnostics.is_empty() => Ok(settled),
        Ok(_) => Err(diagnostics),
        Err(errors) => {
            diagnostics.extend(errors);
            Err(diagnostics)
        }
    }
}

/// Inherited requirements retain their declaring owner, not the selected trait.
/// Top-level requirements use the same projection and settlement operation.
fn requirement_reach(
    typed: &TypedTrees,
    symbol: symbols::SymbolHandle,
) -> Option<(String, bool, language_semantics::ServiceReachRowId)> {
    if let Some((owner, requirement)) = typed.trait_machine_signature(symbol) {
        return Some((
            typed.trait_requirement_callable_identity_text(owner, requirement),
            requirement.service_reach_is_installation_bound,
            requirement.service_reach_row,
        ));
    }
    let requirement = typed.machine_by_symbol(symbol)?;
    if requirement.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement {
        return None;
    }
    Some((
        typed.machine_callable_identity_text(requirement)?,
        requirement.service_reach_is_installation_bound,
        requirement.service_reach_row,
    ))
}
