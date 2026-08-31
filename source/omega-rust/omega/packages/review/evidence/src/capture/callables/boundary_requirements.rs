use super::super::semantics::declarations::top_level_requirement_identity;
use super::external_supply::{
    external_binding_matches_provider_binding, project_external_executable_supply_with_source,
};
use crate::capture::source::ProjectedReviewRow;
use crate::record::{
    PackageReviewExternalBinding, PackageReviewExternalCallableSignature,
    PackageReviewExternalExecutableSupply, PackageReviewExternalRequirement,
    PackageReviewNominalIdentity,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(super) fn project_top_level_requirement_external_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    conformance: &psi_typed_trees::machine::TraitConformance,
    requirement: &psi_typed_trees::machine::Machine,
    callable_identity: &PackageReviewNominalIdentity,
    signature: &PackageReviewExternalCallableSignature,
    binding: &PackageReviewExternalBinding,
) -> Result<ProjectedReviewRow<PackageReviewExternalExecutableSupply>, Vec<Diagnostic>> {
    if conformance.symbol != requirement.symbol
        || conformance.requirement_symbol != requirement.symbol
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` top-level requirement realization `{}` drifted from its retained exact overload",
            machine.name, requirement.name
        ))]);
    }
    if !requirement.is_public {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` realizes non-public top-level requirement `{}` whose complete contract is absent from package review",
            machine.name, requirement.name
        ))]);
    }
    let requirement_parameters = compilation.machine_type_parameters(requirement);
    let realization_parameters = compilation.machine_type_parameters(machine);
    let unsupported_static_parameter = |parameter: &psi_typed_trees::data::TypeParameter| {
        !matches!(
            parameter.kind,
            psi_typed_trees::data::TypeParameterKind::Type
        ) || parameter.bounds != psi_typed_trees::data::DataProperties::default()
    };
    if !requirement.conformance_bounds.is_empty()
        || requirement_parameters
            .iter()
            .any(unsupported_static_parameter)
        || realization_parameters
            .iter()
            .any(unsupported_static_parameter)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` realizes top-level requirement `{}` with static kinds, bounds, or conformances not yet represented by package review",
            machine.name, requirement.name
        ))]);
    }
    psi_validation::revalidate_top_level_requirement_realization(
        &compilation.typed,
        machine,
        requirement,
        conformance,
    )?;
    if conformance.alias.is_some() {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` realizes top-level requirement `{}` through an alias not yet represented by package review",
            machine.name, requirement.name
        ))]);
    }
    if matches!(binding, PackageReviewExternalBinding::CompilerIntrinsic) {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` realizes top-level requirement `{}` through a compiler intrinsic whose closed execution is not yet represented by package review",
            machine.name, requirement.name
        ))]);
    }
    validate_selected_top_level_requirement_external_supply(
        compilation,
        machine,
        requirement,
        binding,
    )?;
    project_external_executable_supply_with_source(
        machine,
        conformance,
        PackageReviewExternalExecutableSupply {
            callable: callable_identity.clone(),
            signature: signature.clone(),
            requirement: PackageReviewExternalRequirement::TopLevelRequirement(
                top_level_requirement_identity(compilation, requirement)?,
            ),
            binding: binding.clone(),
        },
    )
}

pub(super) fn validate_selected_top_level_requirement_external_supply(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    requirement: &psi_typed_trees::machine::Machine,
    binding: &PackageReviewExternalBinding,
) -> Result<(), Vec<Diagnostic>> {
    let plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected top-level-requirement provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = compilation
        .normalized_machine_overload_identity(requirement)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(_plan, retained)| {
            retained.provider.schema
                == omega_provider_planning::plans::ProviderSchemaDeclaration::BoundaryRequirement(
                    requirement.symbol,
                )
                && retained.provider.row_realizations.contains(&machine.symbol)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(());
    }
    let Some(expected_schema) =
        omega_effects::provider_plan::ServiceSchema::from_typed_boundary_requirement(
            &compilation.typed,
            requirement,
        )
    else {
        return Err(vec![Diagnostic::error(format!(
            "selected top-level requirement `{}` has no exact provider schema",
            requirement.name
        ))]);
    };
    let [(plan, retained)] = matches.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed external leaf `{}` realizes top-level requirement `{slot}`, but package review found {} selected provider plans for that exact candidate",
            machine.name,
            matches.len(),
        ))]);
    };
    let [method] = plan.schema.methods.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected top-level-requirement ProviderPlan `{}` must contain exactly one schema method",
            plan.name,
        ))]);
    };
    let [row] = plan.rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected top-level-requirement ProviderPlan `{}` must contain exactly one realization row",
            plan.name,
        ))]);
    };
    let [requirement_symbol] = retained.provider.row_requirements.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected top-level-requirement ProviderPlan `{}` must retain exactly one requirement declaration",
            plan.name,
        ))]);
    };
    let [realization_symbol] = retained.provider.row_realizations.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected top-level-requirement ProviderPlan `{}` must retain exactly one realization declaration",
            plan.name,
        ))]);
    };
    let expected_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.symbol);
    let expected_provider_type = machine
        .attached_data
        .as_ref()
        .map(|name| name.as_str())
        .unwrap_or_default();
    let expected_provider_type_package = compilation
        .typed
        .symbols
        .symbol_package_identity(machine.attached_data_symbol);
    if retained.plan != **plan
        || *requirement_symbol != requirement.symbol
        || *realization_symbol != machine.symbol
        || retained.provider.provider_type != Some(machine.attached_data_symbol)
        || plan.origin_package_identity != expected_package
        || plan.provider_type != expected_provider_type
        || plan.provider_type_package_identity != expected_provider_type_package
        || plan.schema != expected_schema
        || method.requirement_identity != slot
        || row.requirement_identity != slot
        || !external_binding_matches_provider_binding(compilation, machine, binding, &row.binding)
    {
        return Err(vec![Diagnostic::error(format!(
            "selected top-level-requirement ProviderPlan `{}` does not join exact requirement `{slot}` to external leaf `{}` and its binding",
            plan.name, machine.name,
        ))]);
    }
    Ok(())
}
