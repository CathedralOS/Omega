use super::{bindings, rejected};
use crate::capture::providers::installation::project_selected_installation_reach;
use crate::capture::providers::intrinsics::project_compiler_intrinsic_execution;
use crate::capture::providers::selection::validate_selected_provider_declaration_owner;
use crate::capture::semantics::declarations::{
    nominal_identity, provider_requirement_identity, provider_requirement_schema,
};
use crate::record::PackagePolicyProviderRow;
use omega_compiler::CheckedCompilation;
use omega_provider_planning::plans::{ProviderSchemaDeclaration, SelectedProviderReviewProvenance};
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) fn project(
    compilation: &CheckedCompilation,
    target: TargetProfile,
    retained: &SelectedProviderReviewProvenance,
) -> Result<Vec<PackagePolicyProviderRow>, Vec<Diagnostic>> {
    let plan = &retained.plan;
    plan.rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let requirement_symbol = retained.provider.row_requirements[index];
            let realization_symbol = retained.provider.row_realizations[index];
            let declaring_schema = provider_requirement_schema(
                compilation,
                retained.provider.schema,
                requirement_symbol,
            )?;
            let requirement =
                provider_requirement_identity(compilation, declaring_schema, requirement_symbol)?;
            let realization = nominal_identity(compilation, realization_symbol)?;
            validate_selected_provider_declaration_owner(
                &realization,
                plan.origin_package_identity,
                &plan.name,
                "row realization",
            )?;
            validate_lifetime_partition(
                compilation,
                declaring_schema,
                requirement_symbol,
                realization_symbol,
                &row.requirement_lifetime_partition,
            )?;
            Ok(PackagePolicyProviderRow {
                method: row.method.clone(),
                requirement: requirement.clone(),
                realization,
                requirement_lifetime_partition: row.requirement_lifetime_partition.clone(),
                binding: bindings::project(
                    compilation,
                    &row.binding,
                    requirement_symbol,
                    realization_symbol,
                )?,
                compiler_intrinsic_execution: project_compiler_intrinsic_execution(
                    compilation,
                    plan,
                    row,
                    retained.provider.schema,
                    requirement_symbol,
                    realization_symbol,
                    Some(target.target_name()),
                    retained.row_compiler_intrinsic_executions[index],
                )?,
                installation_reach: project_selected_installation_reach(
                    compilation,
                    plan,
                    declaring_schema,
                    requirement_symbol,
                    realization_symbol,
                    &requirement,
                )?,
            })
        })
        .collect()
}

fn validate_lifetime_partition(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
    realization: SymbolHandle,
    retained: &[u32],
) -> Result<(), Vec<Diagnostic>> {
    let ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) = schema else {
        return if retained.is_empty() {
            Ok(())
        } else {
            Err(rejected("non-trait row retains a trait lifetime partition"))
        };
    };
    let machines = compilation
        .machines()
        .iter()
        .filter(|machine| machine.symbol == realization)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(rejected(
            "lifetime partition has no exact realization machine",
        ));
    };
    let applications = compilation
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            conformance.symbol == trait_symbol && conformance.requirement_symbol == requirement
        })
        .collect::<Vec<_>>();
    let [application] = applications.as_slice() else {
        return Err(rejected(
            "lifetime partition has no unique declaring-trait application",
        ));
    };
    if psi_typed_trees::machine::normalize_requirement_lifetime_partition(
        &application.trait_lifetime_arguments,
    ) != retained
    {
        return Err(rejected(
            "row lifetime partition differs from its declaring-trait application",
        ));
    }
    Ok(())
}
