use super::{bindings, rejected};
use crate::capture::PackageReviewInput;
use crate::capture::providers::installation::project_selected_installation_reach;
use crate::capture::providers::intrinsics::project_compiler_intrinsic_execution;
use crate::capture::providers::selection::validate_selected_provider_declaration_owner;
use crate::capture::semantics::declarations::{
    nominal_identity, policy_provider_requirement_identity, provider_requirement_schema,
};
use crate::record::PackagePolicyProviderRow;
use diagnostics::Diagnostic;
use provider_planning::{ProviderSchemaDeclaration, SelectedProviderReviewProvenance};
use symbols::SymbolHandle;
use target::TargetProfile;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    target: TargetProfile,
    retained: &SelectedProviderReviewProvenance,
    toolchain_settled: bool,
    selected_instances: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<PackagePolicyProviderRow>, Vec<Diagnostic>> {
    let plan = &retained.plan;
    plan.rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let requirement_symbol = retained.provider.row_requirements[index];
            let origin = &retained.provider.row_realizations[index];
            let realization_symbol = origin.authored();
            let declaring_schema = provider_requirement_schema(
                compilation,
                retained.provider.schema,
                requirement_symbol,
            )?;
            let requirement = policy_provider_requirement_identity(
                compilation,
                declaring_schema,
                requirement_symbol,
            )?;
            // A toolchain-settled row is realized by the toolchain settlement
            // table, not an authored machine: the provenance retains an
            // `invalid` realization symbol by construction, so the row keeps
            // no realization nominal and no lifetime partition.
            let realization = if toolchain_settled {
                if !matches!(origin, provider_planning::ProviderRowRealization::Toolchain) {
                    return Err(rejected(
                        "toolchain-settled row names an authored realization",
                    ));
                }
                if !row.requirement_lifetime_partition.is_empty() {
                    return Err(rejected(
                        "toolchain-settled row retains a lifetime partition",
                    ));
                }
                None
            } else if origin.native().is_some() {
                None
            } else {
                let realization_symbol = realization_symbol.ok_or_else(|| rejected("provider row lost its authored realization"))?;
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
                    plan,
                    row,
                )?;
                Some(realization)
            };
            let native_build = origin.native().map(|native| bindings::native_producer(compilation, native)).transpose()?;
            let binding = if let Some(native) = origin.native() {
                bindings::project_native(compilation, native, &row.binding)?
            } else {
                bindings::project(compilation, &plan.identities, &row.binding, requirement_symbol,
                    realization_symbol.unwrap_or_else(SymbolHandle::invalid), toolchain_settled)?
            };
            // A native promise has no checked body to substitute for an
            // installation-relative reach. Retain its independent requirement
            // judgment; do not borrow the Build producer's effects.
            let installation_reach = match realization_symbol {
                Some(realization) => project_selected_installation_reach(compilation, selected_instances, plan, declaring_schema,
                    requirement_symbol, realization, &requirement)?,
                None if origin.native().is_some() => {
                    let (_, signature) = compilation.typed.trait_machine_signature(requirement_symbol)
                        .ok_or_else(|| rejected("native row lost its complete requirement contract"))?;
                    if signature.service_reach_is_installation_bound { return Err(rejected("native Build installation-relative reach requires a retained native promise resolution")); }
                    None
                }
                None => None,
            };
            let compiler_intrinsic_execution = match realization_symbol {
                Some(realization) => project_compiler_intrinsic_execution(compilation, plan, row, retained.provider.schema, retained.provider.provider_type,
                    requirement_symbol, realization, Some(target.target_name()), retained.row_compiler_intrinsic_executions[index])?,
                None if retained.row_compiler_intrinsic_executions[index].is_none() => None,
                None => return Err(rejected("native row carries an authored intrinsic execution")),
            };
            Ok(PackagePolicyProviderRow {
                method: row.method.clone(),
                requirement: requirement.clone(),
                realization,
                native_build,
                requirement_lifetime_partition: row.requirement_lifetime_partition.clone(),
                admitted_same_stack_bytes: row.admitted_same_stack_bytes,
                binding,
                compiler_intrinsic_execution,
                installation_reach,
            })
        })
        .collect()
}

fn validate_lifetime_partition(
    compilation: &PackageReviewInput<'_>,
    schema: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
    realization: SymbolHandle,
    plan: &effects::provider_plan::ProviderPlan,
    row: &effects::provider_plan::ProviderPlanRow,
) -> Result<(), Vec<Diagnostic>> {
    let retained = row.requirement_lifetime_partition.as_slice();
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
    let application = provider_planning::provider_trait_application(
        provider_planning::ProviderPlanningContext::new(
            compilation.checking_input,
            source::DependencyScope::Product,
        ),
        plan,
        row,
        machine,
        trait_symbol,
        requirement,
    )
    .ok_or_else(|| rejected("lifetime partition has no exact selected provider application"))?;
    if language_semantics::type_identity::normalize_requirement_lifetime_partition(
        application.lifetime_arguments(),
    ) != retained
    {
        return Err(rejected(
            "row lifetime partition differs from its declaring-trait application",
        ));
    }
    Ok(())
}
