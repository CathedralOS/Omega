use crate::capture::PackageReviewInput;
use crate::capture::behavior::{project_installation_reaches, project_service_row};
use crate::capture::semantics::declarations::{nominal_identity, provider_requirement_schema};
use crate::record::{PackageReviewNominalIdentity, PackageReviewSelectedInstallationReach};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use language_semantics::ServiceReachRowId;
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

mod dependencies;
pub(super) use dependencies::validate_installation_dependency_order;

pub(crate) fn project_selected_installation_reach(
    compilation: &PackageReviewInput<'_>,
    selected: &effects::SelectedProviderPlanFacts,
    plan: &ProviderPlan,
    schema: ProviderSchemaDeclaration,
    requirement_symbol: SymbolHandle,
    realization_symbol: SymbolHandle,
    requirement_identity: &PackageReviewNominalIdentity,
) -> Result<Option<PackageReviewSelectedInstallationReach>, Vec<Diagnostic>> {
    let requirement_reach = match provider_requirement_schema(
        compilation,
        schema,
        requirement_symbol,
    )? {
        ProviderSchemaDeclaration::BoundaryTrait(trait_symbol) => {
            let owners = compilation
                .traits()
                .iter()
                .filter(|owner| owner.symbol == trait_symbol)
                .collect::<Vec<_>>();
            let [owner] = owners.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` installation reach resolves its schema to {} exact traits; expected one",
                    plan.name,
                    owners.len(),
                ))]);
            };
            let requirements = compilation
                .trait_machine_signatures(owner)
                .iter()
                .filter(|requirement| requirement.symbol == requirement_symbol)
                .collect::<Vec<_>>();
            let [requirement] = requirements.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` installation reach resolves `{}` to {} exact requirements; expected one",
                    plan.name,
                    requirement_identity.path(),
                    requirements.len(),
                ))]);
            };
            requirement
                .service_reach_is_installation_bound
                .then_some(requirement.service_reach_row)
        }
        ProviderSchemaDeclaration::BoundaryRequirement(schema_symbol)
        | ProviderSchemaDeclaration::SealedAccessor(schema_symbol) => {
            let requirements = compilation
                .machines()
                .iter()
                .filter(|requirement| {
                    requirement.symbol == schema_symbol
                        && requirement.symbol == requirement_symbol
                        && (requirement.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                            || matches!(schema, ProviderSchemaDeclaration::SealedAccessor(_)))
                })
                .collect::<Vec<_>>();
            let [requirement] = requirements.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected provider plan `{}` installation reach resolves `{}` to {} exact top-level requirements; expected one",
                    plan.name,
                    requirement_identity.path(),
                    requirements.len(),
                ))]);
            };
            requirement
                .service_reach_is_installation_bound
                .then_some(requirement.service_reach_row)
        }
        ProviderSchemaDeclaration::BoundaryOperator(_) => return Ok(None),
    };
    let Some(requirement_reach) = requirement_reach else {
        return Ok(None);
    };

    let retained = selected
        .installation_reach_resolutions()
        .iter()
        .filter(|resolution| {
            selected.identities().text(resolution.requirement_identity)
                == Some(requirement_identity.path())
        })
        .collect::<Vec<_>>();
    let [retained] = retained.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` installation-bound requirement `{}` has {} retained reach resolutions; expected one",
            plan.name,
            requirement_identity.path(),
            retained.len(),
        ))]);
    };
    if retained.provider_plan_report_identity != plan.report_fingerprint() {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` installation-bound requirement `{}` names a different selected plan",
            plan.name,
            requirement_identity.path(),
        ))]);
    }

    let upper_bound_names = exact_service_names(compilation, requirement_reach)?;
    if retained.upper_bound != upper_bound_names {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` installation-bound requirement `{}` has a retained upper bound that disagrees with its exact checked requirement",
            plan.name,
            requirement_identity.path(),
        ))]);
    }

    let reach_facts = compilation
        .facts
        .service_reaches
        .machines()
        .iter()
        .filter(|fact| fact.machine == realization_symbol)
        .collect::<Vec<_>>();
    let [reach_fact] = reach_facts.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` installation-bound requirement `{}` resolves its realization to {} exact service-reach facts; expected one",
            plan.name,
            requirement_identity.path(),
            reach_facts.len(),
        ))]);
    };
    let envelopes = compilation
        .facts
        .contract_plans
        .realized_envelopes
        .iter()
        .filter(|envelope| envelope.machine == realization_symbol)
        .collect::<Vec<_>>();
    let [envelope] = envelopes.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` installation-bound requirement `{}` resolves its realization to {} exact contract envelopes; expected one",
            plan.name,
            requirement_identity.path(),
            envelopes.len(),
        ))]);
    };
    let effective_names = exact_service_names(compilation, reach_fact.effective)?;
    let concrete_names = exact_service_names(compilation, reach_fact.concrete_effective)?;
    let dependencies =
        project_installation_reaches(compilation, &reach_fact.unresolved_installation_reaches)?;
    // The published effective row includes unresolved upper bounds. Review
    // substitutes the checked dependencies under this exact plan, independently
    // of the producer's pending-row iteration. A broad bound is not the selected
    // child's concrete contribution, and cannot replace it in the projection.
    let resolved_names = selected
        .resolve_installation_reach_for_plan(
            &concrete_names,
            &dependencies
                .iter()
                .map(|row| row.requirement().path().to_owned())
                .collect::<Vec<_>>(),
            plan.report_fingerprint(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "selected provider installation reach: {reason}"
            ))]
        })?;
    if envelope.effective_service_reach != effective_names
        || retained.resolved_row != resolved_names
    {
        return Err(vec![Diagnostic::error(format!(
            "selected provider plan `{}` installation-bound requirement `{}` has a resolved reach that disagrees with its exact checked realization",
            plan.name,
            requirement_identity.path(),
        ))]);
    }

    Ok(Some(PackageReviewSelectedInstallationReach {
        upper_bound: project_service_row(compilation, requirement_reach)?,
        resolved: project_service_names(compilation, &resolved_names)?,
    }))
}

fn project_service_names(
    compilation: &PackageReviewInput<'_>,
    names: &[String],
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let services = &compilation.facts.service_reaches.services;
    let mut projected = names
        .iter()
        .map(|name| {
            let definition = services
                .id_for_name(name)
                .and_then(|identity| services.definition(identity))
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected installation reach names an unknown exact service",
                    )]
                })?;
            nominal_identity(compilation, definition.symbol)
        })
        .collect::<Result<Vec<_>, _>>()?;
    projected.sort();
    projected.dedup();
    Ok(projected)
}

fn exact_service_names(
    compilation: &PackageReviewInput<'_>,
    row: ServiceReachRowId,
) -> Result<Vec<String>, Vec<Diagnostic>> {
    let services = compilation.facts.service_reaches.rows.services(row);
    if services.is_empty() && row != language_semantics::ServiceReachRowTable::EMPTY_ROW {
        return Err(vec![Diagnostic::error(
            "selected provider installation reach contains an unknown exact service row",
        )]);
    }
    let mut names = services
        .iter()
        .map(|service| {
            compilation
                .facts
                .service_reaches
                .services
                .definition(*service)
                .map(|definition| definition.name.clone())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "selected provider installation reach contains an unknown exact service",
                    )]
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    names.sort();
    names.dedup();
    Ok(names)
}
