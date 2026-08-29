use super::super::checked_semantics::declarations::{
    nominal_identity, provider_requirement_identity,
};
use super::super::providers::families::project_selected_provider_families;
use super::super::providers::intrinsics::project_compiler_intrinsic_execution;
use super::super::providers::selection::validate_selected_provider_declaration_owner;
use crate::evidence::{
    CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
    CheckedPackageProviderRowIdentity,
};
use omega_compiler::CheckedCompilation;
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;

pub(super) struct ProjectedProviders {
    pub(super) selected: Vec<CheckedPackageProviderReview>,
    pub(super) families: Vec<CheckedPackageProviderFamilyReview>,
}

pub(super) fn project_selected_providers(
    compilation: &CheckedCompilation,
    target: TargetProfile,
) -> Result<ProjectedProviders, Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let selected_provider_provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_provider_provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }

    let mut selected = Vec::with_capacity(selected_plans.len());
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
                Ok(CheckedPackageProviderRowIdentity {
                    requirement: provider_requirement_identity(
                        compilation,
                        retained.provider.schema,
                        *requirement,
                    )?,
                    realization: nominal_identity(compilation, *realization)?,
                    compiler_intrinsic_execution: project_compiler_intrinsic_execution(
                        compilation,
                        plan,
                        row,
                        matches!(
                            retained.provider.schema,
                            omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryOperator(_)
                        ),
                        *requirement,
                        *retained_execution,
                    )?,
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
        selected.push(CheckedPackageProviderReview {
            plan_name: plan.name.clone(),
            plan_report_fingerprint: plan.report_fingerprint(),
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

    let families = project_selected_provider_families(compilation, target, &selected)?;
    Ok(ProjectedProviders { selected, families })
}
