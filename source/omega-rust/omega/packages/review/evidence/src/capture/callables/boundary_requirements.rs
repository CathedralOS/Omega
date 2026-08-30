use super::external_supply::external_binding_matches_provider_binding;
use crate::record::PackageReviewExternalBinding;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

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
    let Some(expected_schema) =
        omega_effects::provider_plan::ServiceSchema::from_typed_boundary_requirement(
            &compilation.typed,
            requirement,
        )
    else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed top-level requirement `{}` has no exact provider schema",
            requirement.name
        ))]);
    };
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
