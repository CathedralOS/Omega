//! Join selected external realizations to canonical satisfaction and exact owners.

use effects::external_providers::matches_external_selection;
pub(super) use effects::external_providers::reconcile_numeric_meaning;

/// Rejoin exact selecting owners at native proposal ingress, including private
/// record recovery. Later execution borrows this same immutable catalog and
/// selected plan; it does not need another parallel declaration sidecar.
pub fn validate_external_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), String> {
    validate_provider_owners(external_providers, identities, declarations, |_, _| true)
}

/// Invocation code retains the complete declarations of its emitted callable
/// closure, not every available external leaf. Keep whole selected plans and
/// check every declaration the module retains, including calls in uncalled
/// dynamic-family rows. A current selector or caller does not narrow this set.
pub fn validate_retained_external_provider_owners(
    module: &terminal_psi::TerminalModule,
    selected: &effects::SelectedProviderPlanFacts,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
) -> Result<(), String> {
    backend_plan::require_selected_provider_declarations(selected, declarations)?;
    let mut retained = module
        .boundary_machines
        .iter()
        .map(|boundary| {
            module
                .identities
                .text(boundary.identity)
                .filter(|identity| !identity.is_empty())
                .ok_or_else(|| {
                    format!(
                        "retained Terminal boundary {:?} has no callable identity",
                        boundary.id,
                    )
                })
        })
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    for demand in &module.boundary_application_demands {
        let identity = demand.requirement().overload();
        if identity.is_empty() {
            return Err("retained boundary application has no callable identity".into());
        }
        retained.insert(identity);
    }
    validate_provider_owners(
        &module.external_providers,
        &module.identities,
        declarations,
        |plan, row| {
            retained.iter().any(|requirement| {
                selected.selected_row_matches_requirement(
                    requirement,
                    plan.identities.text(row.requirement_identity),
                    plan.report_fingerprint(),
                )
            })
        },
    )
}

fn validate_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    declarations: &[backend_plan::TerminalTargetProviderPlan],
    requires_declaration: impl Fn(
        &effects::provider_plan::ProviderPlan,
        &effects::provider_plan::ProviderPlanRow,
    ) -> bool,
) -> Result<(), String> {
    for selected in declarations {
        if selected.row_numeric_realizations.len() != selected.plan.rows.len() {
            return Err("selected external declarations have incomplete row custody".into());
        }
        for (row, realization) in selected
            .plan
            .rows
            .iter()
            .zip(&selected.row_numeric_realizations)
        {
            let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } =
                &row.binding
            else {
                if realization.is_some() {
                    return Err("non-intrinsic row substituted an external realization".into());
                }
                continue;
            };
            if realization.as_ref().is_some_and(String::is_empty) {
                return Err("selected external realization has empty declaration identity".into());
            }
            let mut found = false;
            for candidate in external_providers.iter().filter(|candidate| {
                matches_external_selection(candidate, identities, &selected.plan, row, machine)
                    && realization.as_ref().is_none_or(|realization| {
                        identities.text(candidate.realization.machine.identity())
                            == Some(realization.as_str())
                    })
            }) {
                found = true;
                let owner = candidate
                    .provider
                    .as_ref()
                    .and_then(|provider| identities.text(provider.declaration.identity()));
                if owner != selected.provider_declaration.as_deref() {
                    return Err(
                        "selected external provider has a different exact declaring owner".into(),
                    );
                }
            }
            if realization.is_some() && !found && requires_declaration(&selected.plan, row) {
                return Err(format!(
                    "selected provider `{}` has no retained external implementation for `{}` (implementation `{}`)",
                    selected.plan.provider_type,
                    selected.plan.identities.text(row.requirement_identity),
                    realization.as_deref().unwrap_or_default(),
                ));
            }
        }
    }
    Ok(())
}
