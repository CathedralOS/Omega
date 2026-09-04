use super::super::providers::application_realizations::{
    ProjectedBoundaryApplicationRealizations, project_boundary_application_realizations,
};
use super::super::providers::families::project_selected_provider_families;
use super::super::providers::installation::project_selected_installation_reach;
use super::super::providers::intrinsics::project_compiler_intrinsic_execution;
use super::super::providers::selection::validate_selected_provider_declaration_owner;
use super::super::providers::symbolic_demands::{
    ProjectedBoundaryApplicationDemands, project_boundary_application_demands,
};
use super::super::semantics::declarations::{nominal_identity, provider_requirement_identity};
use crate::record::{
    CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
    CheckedPackageProviderRowIdentity, PackageReviewProviderGrantSelectorKind,
    PackageReviewSelectedProviderGrant,
};
use omega_compiler::CheckedCompilation;
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;

pub(super) struct ProjectedProviders {
    pub(super) selected: Vec<CheckedPackageProviderReview>,
    pub(super) families: Vec<CheckedPackageProviderFamilyReview>,
    pub(super) application_realizations: ProjectedBoundaryApplicationRealizations,
    pub(super) application_demands: ProjectedBoundaryApplicationDemands,
}

pub(super) fn project_selected_providers(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    package: psi_core::PackageKeyIdentity,
) -> Result<ProjectedProviders, Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let selected_provider_provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_provider_provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }
    let selected_provider_grants = compilation.selected_provider_grants();
    if !selected_provider_grants.is_empty() {
        let Some(build_machine) = compilation.selected_build_machine_symbol() else {
            return Err(vec![Diagnostic::error(
                "selected-provider grants have no exact selected build machine",
            )]);
        };
        for grant in selected_provider_grants {
            if grant.selecting_machine != build_machine {
                return Err(vec![Diagnostic::error(format!(
                    "selected-provider grant `{}` was not authored by the selected build machine",
                    grant.grant.selector,
                ))]);
            }
            let matches = selected_plans
                .iter()
                .filter(|plan| grant.grant.replays_selected_plan(plan))
                .count();
            if matches != 1 {
                return Err(vec![Diagnostic::error(format!(
                    "selected-provider grant `{}` rejoins {matches} exact selected plans",
                    grant.grant.selector,
                ))]);
            }
        }
    }

    let mut selected = Vec::with_capacity(selected_plans.len());
    let mut projected_installation_reaches = 0usize;
    let mut projected_provider_grants = 0usize;
    for (plan, retained) in selected_plans.iter().zip(selected_provider_provenance) {
        if retained.plan != *plan
            || retained.provider.row_requirements.len() != plan.rows.len()
            || retained.provider.row_realizations.len() != plan.rows.len()
            || retained.row_compiler_intrinsic_executions.len() != plan.rows.len()
        {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` has incomplete or misaligned declaration provenance",
                plan.name,
            ))]);
        }
        let row_declarations = retained
            .provider
            .row_requirements
            .iter()
            .zip(&retained.provider.row_realizations)
            .zip(&retained.row_compiler_intrinsic_executions)
            .zip(&plan.rows)
            .map(|(((requirement, realization), retained_execution), row)| {
                validate_selected_requirement_lifetime_partition(
                    compilation,
                    plan,
                    row,
                    *requirement,
                    *realization,
                )?;
                let requirement_identity = provider_requirement_identity(
                    compilation,
                    retained.provider.schema,
                    *requirement,
                )?;
                let installation_reach = project_selected_installation_reach(
                    compilation,
                    plan,
                    retained.provider.schema,
                    *requirement,
                    *realization,
                    &requirement_identity,
                )?;
                projected_installation_reaches += usize::from(installation_reach.is_some());
                Ok(CheckedPackageProviderRowIdentity {
                    requirement: requirement_identity,
                    realization: nominal_identity(compilation, *realization)?,
                    compiler_intrinsic_execution: project_compiler_intrinsic_execution(
                        compilation,
                        plan,
                        row,
                        retained.provider.schema,
                        *requirement,
                        *realization,
                        Some(target.target_name()),
                        *retained_execution,
                    )?,
                    installation_reach,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
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
        match provider_type_declaration.as_ref() {
            Some(declaration) => validate_selected_provider_declaration_owner(
                declaration,
                plan.provider_type_package_identity,
                &plan.name,
                "provider type",
            )?,
            None if plan.provider_type.is_empty()
                && plan.provider_type_package_identity.is_none() => {}
            None => {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` has provider-type identity without one exact declaration",
                    plan.name,
                ))]);
            }
        }
        for (row, declarations) in plan.rows.iter().zip(&row_declarations) {
            let mut methods = plan
                .schema
                .methods
                .iter()
                .filter(|method| method.requirement_identity == row.requirement_identity);
            let Some(method) = methods.next() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` row `{}` has no exact schema method",
                    plan.name, row.requirement_identity,
                ))]);
            };
            if methods.next().is_some() {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` row `{}` has duplicate schema methods",
                    plan.name, row.requirement_identity,
                ))]);
            }
            validate_selected_provider_declaration_owner(
                &declarations.requirement,
                method.requirement_owner_package_identity,
                &plan.name,
                "row requirement",
            )?;
            validate_selected_provider_declaration_owner(
                &declarations.realization,
                plan.origin_package_identity,
                &plan.name,
                "row realization",
            )?;
        }
        let mut grants = selected_provider_grants
            .iter()
            .filter(|grant| grant.grant.replays_selected_plan(plan))
            .map(|grant| PackageReviewSelectedProviderGrant {
                selector_kind: match grant.grant.selector_kind {
                    omega_trust_model::ProviderGrantSelectorKind::PlanName => {
                        PackageReviewProviderGrantSelectorKind::PlanName
                    }
                    omega_trust_model::ProviderGrantSelectorKind::ProviderSlot => {
                        PackageReviewProviderGrantSelectorKind::ProviderSlot
                    }
                },
                selected_plan_digest: *grant.grant.selected_plan_digest.as_bytes(),
            })
            .collect::<Vec<_>>();
        grants.sort();
        if grants.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` has duplicate exact authored grants",
                plan.name,
            ))]);
        }
        projected_provider_grants += grants.len();
        selected.push(CheckedPackageProviderReview {
            plan_name: plan.name.clone(),
            plan_report_fingerprint: plan.report_fingerprint(),
            grants,
            realizing_package: plan.origin_package_identity,
            schema_declaration,
            provider_type: plan.provider_type.clone(),
            provider_type_package: plan.provider_type_package_identity,
            provider_type_declaration,
            schema: plan.schema.clone(),
            target: plan.target.clone(),
            rows: plan.rows.clone(),
            row_declarations,
        });
    }

    if projected_provider_grants != selected_provider_grants.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider review contains an orphan authored provider grant",
        )]);
    }

    if projected_installation_reaches
        != compilation
            .selected_provider_plans()
            .installation_reach_resolutions()
            .len()
    {
        return Err(vec![Diagnostic::error(
            "selected-provider review contains an orphan installation-reach resolution",
        )]);
    }

    let families = project_selected_provider_families(compilation, target, &selected)?;
    let application_realizations = project_boundary_application_realizations(compilation, package)?;
    let application_demands = project_boundary_application_demands(compilation, package)?;
    Ok(ProjectedProviders {
        selected,
        families,
        application_realizations,
        application_demands,
    })
}

fn validate_selected_requirement_lifetime_partition(
    compilation: &CheckedCompilation,
    plan: &omega_effects::provider_plan::ProviderPlan,
    row: &omega_effects::provider_plan::ProviderPlanRow,
    requirement: psi_symbols::SymbolHandle,
    realization: psi_symbols::SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    let declaring_traits = compilation
        .traits()
        .iter()
        .filter(|definition| {
            compilation
                .trait_machine_signatures(definition)
                .iter()
                .any(|candidate| candidate.symbol == requirement)
        })
        .collect::<Vec<_>>();
    let ([] | [_]) = declaring_traits.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` requirement `{}` belongs to {} exact traits",
            plan.name,
            row.requirement_identity,
            declaring_traits.len(),
        ))]);
    };
    let [] = declaring_traits.as_slice() else {
        let [definition] = declaring_traits.as_slice() else {
            unreachable!("trait declaration cardinality checked above")
        };
        let machines = compilation
            .machines()
            .iter()
            .filter(|machine| machine.symbol == realization)
            .collect::<Vec<_>>();
        let [machine] = machines.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` row `{}` realization has {} exact typed machines",
                plan.name,
                row.requirement_identity,
                machines.len(),
            ))]);
        };
        let applications = compilation
            .machine_trait_conformances(machine)
            .iter()
            .filter(|conformance| {
                conformance.symbol == definition.symbol
                    && conformance.requirement_symbol == requirement
            })
            .collect::<Vec<_>>();
        let [application] = applications.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` row `{}` realization retains {} exact requirement lifetime applications",
                plan.name,
                row.requirement_identity,
                applications.len(),
            ))]);
        };
        let replayed = psi_typed_trees::machine::normalize_requirement_lifetime_partition(
            &application.trait_lifetime_arguments,
        );
        if replayed != row.requirement_lifetime_partition {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` row `{}` requirement lifetime partition differs from its exact realization edge",
                plan.name, row.requirement_identity,
            ))]);
        }
        return Ok(());
    };
    if row.requirement_lifetime_partition.is_empty() {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` non-trait row `{}` retains a target-trait lifetime partition",
            plan.name, row.requirement_identity,
        ))])
    }
}
