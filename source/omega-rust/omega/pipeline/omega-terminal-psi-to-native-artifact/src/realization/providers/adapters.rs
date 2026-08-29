use std::collections::BTreeSet;

use omega_effects::provider_plan::ProviderBinding;
use omega_psi_to_abstract_operations::SelectedProviderAdapter;

/// Project only checked, in-artifact provider adapters from the selected
/// provider closure. External bindings continue through provider-execution
/// settlements; they must never be reinterpreted as checked Omega machines.
pub(crate) fn project_selected_provider_adapters(
    selected: &omega_effects::SelectedProviderPlanFacts,
    terminal: &omega_abstract_operations::AbstractOperationPlan,
) -> Result<Vec<SelectedProviderAdapter>, String> {
    let relevant_requirements = terminal
        .provider_candidates
        .iter()
        .map(|candidate| candidate.requirement_identity.as_str())
        .collect::<BTreeSet<_>>();
    project_selected_provider_adapters_for_requirements(selected, &relevant_requirements)
}

pub(crate) fn project_selected_provider_adapters_for_requirements(
    selected: &omega_effects::SelectedProviderPlanFacts,
    relevant_requirements: &BTreeSet<&str>,
) -> Result<Vec<SelectedProviderAdapter>, String> {
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
            if !relevant_requirements.contains(row.requirement_identity.as_str()) {
                continue;
            }
            if plan.provider_type.is_empty() {
                return Err(format!(
                    "selected ProviderPlan `{}` has a checked adapter but no exact provider type identity",
                    plan.name
                ));
            }
            if row.requirement_identity.is_empty() || machine_identity.is_empty() {
                return Err(format!(
                    "selected ProviderPlan `{}` has an incomplete checked-adapter identity",
                    plan.name
                ));
            }
            if *machine_package_identity != plan.origin_package_identity {
                return Err(format!(
                    "selected checked adapter `{machine_identity}` for ProviderPlan `{}` drifted from the plan's exact origin package",
                    plan.name
                ));
            }
            adapters.push(SelectedProviderAdapter {
                requirement_identity: row.requirement_identity.clone(),
                provider_identity: plan.provider_type.clone(),
                machine_identity: machine_identity.clone(),
            });
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
