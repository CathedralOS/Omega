use std::collections::BTreeSet;

use effects::provider_plan::ProviderBinding;
use terminal_psi_to_abstract_operations::SelectedProviderAdapter;

/// Project only checked, in-artifact provider adapters from the selected
/// provider closure. External bindings continue through provider-execution
/// settlements; they must never be reinterpreted as checked Omega machines.
pub fn project_selected_provider_adapters(
    selected: &effects::SelectedProviderPlanFacts,
    terminal: &abstract_operations::AbstractOperationPlan,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let relevant_requirements = terminal
        .provider_candidates
        .iter()
        .map(|candidate| candidate.requirement_identity.as_str())
        .collect::<BTreeSet<_>>();
    project_selected_provider_adapters_for_requirements(selected, &relevant_requirements)
}

pub fn project_selected_provider_adapters_for_requirements(
    selected: &effects::SelectedProviderPlanFacts,
    relevant_requirements: &BTreeSet<&str>,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    fn project(
        plan: &effects::provider_plan::ProviderPlan,
        relevant_requirements: &BTreeSet<&str>,
        requirement_identity: &str,
        machine_identity: &str,
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
        adapters: &mut Vec<SelectedProviderAdapter>,
    ) -> Result<(), String> {
        if !relevant_requirements.contains(requirement_identity) {
            return Ok(());
        }
        if plan.provider_type.is_empty() {
            return Err(format!(
                "selected ProviderPlan `{}` has a checked adapter but no exact provider type identity",
                plan.name
            ));
        }
        if requirement_identity.is_empty() || machine_identity.is_empty() {
            return Err(format!(
                "selected ProviderPlan `{}` has an incomplete checked-adapter identity",
                plan.name
            ));
        }
        if machine_package_identity != plan.origin_package_identity {
            return Err(format!(
                "selected checked adapter `{machine_identity}` for ProviderPlan `{}` drifted from the plan's exact origin package",
                plan.name
            ));
        }
        adapters.push(SelectedProviderAdapter {
            requirement_identity: requirement_identity.to_owned(),
            provider_identity: plan.provider_type.clone(),
            machine_identity: machine_identity.to_owned(),
        });
        Ok(())
    }

    let mut adapters = Vec::new();
    for plan in selected.plans() {
        for row in &plan.rows {
            let ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } = &row.binding
            else {
                continue;
            };
            // A template-keyed selected row serves its closed requirement
            // instances through the specialized provider instances the
            // roster retains. A row the roster never rebinds keeps its own
            // exact identity.
            let specialized = selected
                .specialized_requirement_rows_for_row(
                    plan.report_fingerprint(),
                    &row.requirement_identity,
                )
                .collect::<Vec<_>>();
            if specialized.is_empty() {
                project(
                    plan,
                    relevant_requirements,
                    &row.requirement_identity,
                    machine_identity,
                    *machine_package_identity,
                    &mut adapters,
                )?;
            } else {
                for specialization in specialized {
                    project(
                        plan,
                        relevant_requirements,
                        &specialization.requirement_identity,
                        &specialization.machine_identity,
                        *machine_package_identity,
                        &mut adapters,
                    )?;
                }
            }
        }
    }
    adapters.sort_by(|left, right| {
        (
            &left.requirement_identity,
            &left.provider_identity,
            &left.machine_identity,
        )
            .cmp(&(
                &right.requirement_identity,
                &right.provider_identity,
                &right.machine_identity,
            ))
    });
    if let Some(duplicate) = adapters
        .windows(2)
        .find(|rows| rows[0].requirement_identity == rows[1].requirement_identity)
    {
        return Err(format!(
            "selected provider closure projects more than one checked adapter for exact requirement `{}`",
            duplicate[0].requirement_identity
        ));
    }
    Ok(adapters)
}
