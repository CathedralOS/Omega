use provider_correspondence::external_providers::matches_external_selection;
use provider_plans::ProviderPlan;
use std::collections::BTreeSet;

// A selected external realization needs its exact declaring owner only where
// the module retains the requirement as a boundary or boundary application:
// native realization and intrinsic admission both execute selected rows by that
// demand, so a selected row nothing retains is never realized. Every consumer
// shares this one demand contract rather than asking for declarations of rows
// the program never executes.
pub fn validate_external_provider_owners(
    module: &terminal_psi::TerminalModule,
    selected: &selected_provider_plans::SelectedProviderPlanFacts,
) -> Result<(), String> {
    let mut retained = module
        .interfaces.boundary_machines
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
        .collect::<Result<BTreeSet<_>, _>>()?;
    for demand in &module.interfaces.boundary_application_demands {
        let identity = demand.requirement().overload();
        if identity.is_empty() {
            return Err("retained boundary application has no callable identity".into());
        }
        retained.insert(identity);
    }
    validate_retained_provider_owners(
        &module.interfaces.external_providers,
        &module.identities,
        selected,
        &retained,
    )
}

pub(crate) fn validate_retained_provider_owners(
    external_providers: &[terminal_psi::ExternalProviderConformance],
    identities: &terminal_psi::TerminalIdentityTable,
    facts: &selected_provider_plans::SelectedProviderPlanFacts,
    retained: &BTreeSet<&str>,
) -> Result<(), String> {
    let requires_declaration = |plan: &ProviderPlan, row: &provider_plans::ProviderPlanRow| {
        retained.iter().any(|requirement| {
            facts.selected_row_matches_requirement(
                requirement,
                plan.identities.text(row.requirement_identity),
                plan.report_fingerprint(),
            )
        })
    };
    for selected in facts.plans() {
        if selected.row_numeric_realizations.len() != selected.rows.len() {
            return Err("selected external declarations have incomplete row custody".into());
        }
        for (row, realization) in selected.rows.iter().zip(&selected.row_numeric_realizations) {
            let provider_plans::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
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
                matches_external_selection(candidate, identities, selected, row, machine)
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
            if realization.is_some() && !found && requires_declaration(selected, row) {
                return Err(format!(
                    "selected provider `{}` has no retained external implementation for `{}` (implementation `{}`)",
                    selected.provider_type,
                    selected.identities.text(row.requirement_identity),
                    realization.as_deref().unwrap_or_default(),
                ));
            }
        }
    }
    Ok(())
}
