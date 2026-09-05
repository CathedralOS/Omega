//! Complete selected-provider policy captured from live checked associations.

mod bindings;
mod families;
mod replay;
mod rows;

use crate::capture::providers::selection::validate_selected_provider_declaration_owner;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::services;
use crate::record::{
    PackagePolicyProviderPlan, PackagePolicySelectedProviders,
    PackageReviewProviderGrantSelectorKind,
};
use omega_compiler::CheckedCompilation;
use omega_provider_planning::plans::SelectedProviderReviewProvenance;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

/// Retain the root activation's complete selected provider meaning without
/// evaluator, calling, native, or admission receipts. This grants no authority.
pub fn project_checked_selected_provider_policy(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicySelectedProviders, Vec<Diagnostic>> {
    replay::validate(compilation, target, package)?;
    let mut plans = compilation
        .selected_provider_provenance()
        .iter()
        .enumerate()
        .map(|(index, retained)| {
            project_plan(compilation, target, retained).map(|plan| (index, plan))
        })
        .collect::<Result<Vec<_>, _>>()?;
    plans.sort_by(|(_, left), (_, right)| left.compare_canonical(right));
    let families = families::project(compilation, target, &plans)?;
    let projected_installation_reaches = plans
        .iter()
        .flat_map(|(_, plan)| &plan.rows)
        .filter(|row| row.installation_reach.is_some())
        .count();
    if projected_installation_reaches
        != compilation
            .selected_provider_plans()
            .installation_reach_resolutions()
            .len()
    {
        return Err(rejected(
            "an installation-reach resolution has no exact selected row",
        ));
    }
    let projected_grants: usize = plans.iter().map(|(_, plan)| plan.grants.len()).sum();
    if projected_grants != compilation.selected_provider_grants().len() {
        return Err(rejected(
            "an authored provider grant has no unique selected plan",
        ));
    }
    let policy = PackagePolicySelectedProviders {
        package,
        target,
        plans: plans.into_iter().map(|(_, plan)| plan).collect(),
        families,
    };
    policy.validate_canonical_structure().map_err(rejected)?;
    Ok(policy)
}

fn project_plan(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    retained: &SelectedProviderReviewProvenance,
) -> Result<PackagePolicyProviderPlan, Vec<Diagnostic>> {
    let plan = &retained.plan;
    let schema_declaration = nominal_identity(compilation, retained.provider.schema.symbol())?;
    validate_selected_provider_declaration_owner(
        &schema_declaration,
        plan.schema.trait_package_identity,
        &plan.name,
        "service schema",
    )?;
    let provider_type_declaration = retained
        .provider
        .provider_type
        .map(|symbol| nominal_identity(compilation, symbol))
        .transpose()?;
    match &provider_type_declaration {
        Some(declaration) => validate_selected_provider_declaration_owner(
            declaration,
            plan.provider_type_package_identity,
            &plan.name,
            "provider type",
        )?,
        None if plan.provider_type.is_empty() && plan.provider_type_package_identity.is_none() => {}
        None => return Err(rejected("provider type has no exact declaration")),
    }
    let mut methods = Vec::with_capacity(plan.schema.methods.len());
    for method in &plan.schema.methods {
        let matching_rows = plan
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.requirement_identity == method.requirement_identity)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [index] = matching_rows.as_slice() else {
            return Err(rejected(
                "service method has no unique selected realization row",
            ));
        };
        methods.push(services::project(
            compilation,
            retained.provider.schema,
            retained.provider.provider_type,
            retained.provider.row_requirements[*index],
            method,
        )?);
    }
    let mut rows = rows::project(compilation, target, retained)?;
    rows.sort_by(|left, right| left.requirement.cmp(&right.requirement));
    let mut grants = compilation
        .selected_provider_grants()
        .iter()
        .filter(|grant| grant.grant.replays_selected_plan(plan))
        .map(|grant| match grant.grant.selector_kind {
            omega_trust_model::ProviderGrantSelectorKind::PlanName => {
                PackageReviewProviderGrantSelectorKind::PlanName
            }
            omega_trust_model::ProviderGrantSelectorKind::ProviderSlot => {
                PackageReviewProviderGrantSelectorKind::ProviderSlot
            }
        })
        .collect::<Vec<_>>();
    grants.sort();
    if grants.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(rejected(
            "selected plan has duplicate exact grant selectors",
        ));
    }
    Ok(PackagePolicyProviderPlan {
        plan_name: plan.name.clone(),
        realizing_package: plan.origin_package_identity,
        schema_declaration,
        provider_type: plan.provider_type.clone(),
        provider_type_declaration,
        target: plan.target.clone(),
        methods,
        rows,
        grants,
    })
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "selected-provider policy rejects {reason}"
    ))]
}
