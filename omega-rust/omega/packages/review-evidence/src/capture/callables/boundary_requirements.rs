//! Top-level requirement associations retain machine overload identity.
//! Checked bodies are revalidated against their requirement; external leaves
//! retain separate executable-supply findings. Neither association chooses a
//! provider: selected-provider policy independently replays that decision.

use super::super::semantics::declarations::top_level_requirement_identity;
use super::external_supply::external_binding_matches_provider_binding;
use crate::capture::PackageReviewInput;
use crate::record::{PackageReviewConformanceBound, PackageReviewExternalBinding};
use diagnostics::Diagnostic;

pub(super) fn project_checked_requirement_realization(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    conformance: &typed_trees::machine::TraitConformance,
    requirement: &typed_trees::machine::Machine,
) -> Result<crate::record::PackageReviewRequirementRealization, Vec<Diagnostic>> {
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{}` realizes top-level requirement `{}` without one checked implementation body",
            machine.name, requirement.name,
        ))]);
    }
    if machine.is_public && !requirement.is_public {
        return Err(vec![Diagnostic::error(format!(
            "reviewed public callable `{}` exposes private top-level requirement `{}`",
            machine.name, requirement.name,
        ))]);
    }
    validation::revalidate_top_level_requirement_realization(
        &compilation.typed,
        machine,
        requirement,
        conformance,
    )?;
    Ok(crate::record::PackageReviewRequirementRealization {
        requirement_identity: top_level_requirement_identity(compilation, requirement)?,
        alias: conformance
            .alias
            .as_ref()
            .map(|alias| alias.as_str().to_owned()),
    })
}

pub(super) fn provider_conformance_bounds_refine(
    requirement: &[PackageReviewConformanceBound],
    provider: &[PackageReviewConformanceBound],
) -> bool {
    let mut matched = vec![false; requirement.len()];
    provider.iter().all(|provider_bound| {
        let Some((index, _)) = requirement
            .iter()
            .enumerate()
            .find(|(index, requirement_bound)| {
                !matched[*index] && same_conformance_bound_shape(requirement_bound, provider_bound)
            })
        else {
            return false;
        };
        matched[index] = true;
        true
    })
}

fn same_conformance_bound_shape(
    requirement: &PackageReviewConformanceBound,
    provider: &PackageReviewConformanceBound,
) -> bool {
    requirement.trait_identity == provider.trait_identity
        && requirement.trait_lifetime_arguments == provider.trait_lifetime_arguments
        && requirement.arguments == provider.arguments
}

pub(super) fn validate_selected_top_level_requirement_external_supply(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    requirement: &typed_trees::machine::Machine,
    binding: &PackageReviewExternalBinding,
) -> Result<(), Vec<Diagnostic>> {
    let selected_execution = compilation.custody.execution();
    let plans = selected_execution.selected_provider_plan_facts.plans();
    let provenance = selected_execution.selected_provider_provenance.as_slice();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "selected top-level-requirement provider plans are not aligned with retained declaration provenance",
        )]);
    }
    let slot = compilation
        .machine_callable_identity_text(requirement)
        .unwrap_or_default();
    let matches = plans
        .iter()
        .zip(provenance)
        .filter(|(_plan, retained)| {
            retained.provider.schema
                == provider_planning::ProviderSchemaDeclaration::BoundaryRequirement(
                    requirement.symbol,
                )
                && retained.provider.row_realizations.contains(&machine.symbol)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Ok(());
    }
    let Some(expected_schema) = provider_planning::service_schema::from_typed_boundary_requirement(
        provider_planning::ProviderPlanningContext::new(
            &compilation.typed,
            source::DependencyScope::Product,
        ),
        requirement,
    ) else {
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
    let expected_provider_type = compilation
        .typed
        .attached_data_path(machine)
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
