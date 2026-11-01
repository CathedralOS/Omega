//! Choose checked semantic execution before evaluating native calling plans.
//!
//! A policy may call a pure boundary helper selected by Build. Selecting its
//! checked declaration needs the neutral service contract, not the ABI value
//! that the policy is computing. Ordinary derivation and whole-map selection
//! own that choice; final settlement later attaches evaluated calling evidence.
//! These provisional commitments never become native calling identities.

use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
use language_semantics::direct_calls::DirectCallDeclarations;
use std::collections::HashSet;
use symbols::SymbolHandle;

pub struct CallingPolicyProviderInputs<'program> {
    pub body_declarations: &'program DirectCallDeclarations<SymbolHandle>,
    pub defaults: &'program [crate::ProviderSelection],
    pub requested: &'program [crate::ProviderSelection],
    pub exact_toolchain_sources: &'program [(source::SourceId, [u8; 32])],
}

pub(super) struct SelectedPolicyProviders {
    pub bodies: Vec<build_time_evaluation::SelectedBuildTimeProviderBody>,
    pub declarations: Vec<backend_plan::TerminalTargetProviderPlan>,
}

pub(super) fn prepare(
    checked: &checked_trees::CheckedTrees,
    services: ServiceCatalog<'_>,
    target: target::TargetProfile,
    roots: &[SymbolHandle],
    machine_bodies: &[build_time_evaluation::SelectedBuildTimeMachineBody],
    inputs: &CallingPolicyProviderInputs<'_>,
) -> Result<SelectedPolicyProviders, Vec<Diagnostic>> {
    let typed = checked.typed.as_ref();
    let candidates = crate::derive_satisfies_plans(crate::ProviderPlanDerivation::unevaluated(
        services,
        inputs.body_declarations,
        &[],
        Some(target.target_name()),
    ))?;
    let graph = build_time_evaluation::BuildTimeAdmissionPlan::infer(typed, None)
        .with_selected_machine_bodies(typed, machine_bodies)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let mut roots = roots.to_vec();
    let mut prior_demands = HashSet::new();
    let selected = loop {
        let closure = graph.execution_dependencies(typed, &roots);
        let demands: HashSet<_> = checked
            .facts
            .operators
            .resolved_expression_selections()
            .filter(|(_, origin, _, _)| {
                origin
                    .machine_symbol()
                    .is_some_and(|owner| closure.contains(&owner))
            })
            .map(|(_, _, requirement, _)| requirement)
            .filter(|requirement| is_boundary_requirement(typed, *requirement))
            .collect();
        let demanded = candidates
            .iter()
            .filter(|candidate| {
                candidate
                    .provenance
                    .row_requirements
                    .iter()
                    .any(|requirement| demands.contains(requirement))
            })
            .cloned()
            .collect::<Vec<_>>();
        // Unrelated authored/native supplies stay with ordinary settlement.
        // Keep every choice for a demanded schema, including invalid choices:
        // filtering by provider availability would silently discard a refusal.
        let relevant = |choice: &&crate::ProviderSelection| {
            demanded.iter().any(|candidate| {
                candidate.plan.rows.iter().any(|row| {
                    choice.subject.selects_schema(
                        candidate.provenance.schema.symbol(),
                        candidate.plan.identity_text(row.requirement_identity),
                        candidate.plan.schema.root_application_identity(),
                    )
                })
            })
        };
        let defaults = inputs
            .defaults
            .iter()
            .filter(relevant)
            .cloned()
            .collect::<Vec<_>>();
        let requested = inputs
            .requested
            .iter()
            .filter(relevant)
            .cloned()
            .collect::<Vec<_>>();
        let selected =
            crate::select_derived_provider_plans(&demanded, target, &defaults, &requested)?;
        for choice in &selected {
            if !matches!(
                choice.selected_by,
                crate::ProviderSelectionProvenance::BuildOverride(_)
                    | crate::ProviderSelectionProvenance::TargetDefault(_)
            ) {
                return Err(vec![Diagnostic::error(
                    "calling-policy helper requires an exact Build or target-default provider selection",
                )]);
            }
        }
        let plans = selected
            .iter()
            .map(|selected| selected.derived.plan.clone())
            .collect::<Vec<_>>();
        for requirement in &demands {
            let (_, plan) =
                crate::selected_requirement_plan(services.context(), &plans, *requirement)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "calling-policy helper has no exact selected checked provider",
                        )]
                    })?;
            let (body, _) = crate::selected_requirement_checked_adapter(services, plan, *requirement)
                .map_err(|error| vec![error])?
                .ok_or_else(|| vec![Diagnostic::error("calling-policy evaluation requires a checked helper body; native supply is not semantic execution")])?;
            if !roots.contains(&body.symbol) {
                roots.push(body.symbol);
            }
        }
        if demands == prior_demands {
            break selected;
        }
        prior_demands = demands;
    };
    let (plans, provenance) = crate::selected_provider_plan_facts(services.context(), selected)?;
    let closure = graph.execution_dependencies(typed, &roots);
    let mut bodies = Vec::new();
    for (expression, origin, requirement, _) in
        checked.facts.operators.resolved_expression_selections()
    {
        if !origin
            .machine_symbol()
            .is_some_and(|owner| closure.contains(&owner))
        {
            continue;
        }
        let Some((_, plan)) =
            crate::selected_requirement_plan(services.context(), plans.plans(), requirement)
        else {
            continue;
        };
        if let Some(body) = crate::prepare_selected_build_time_provider_body(
            services,
            target,
            plan,
            expression,
            origin,
            requirement,
        )
        .map_err(|error| vec![error])?
        {
            bodies.push(body);
        }
    }
    let declarations = crate::prepare_native_provider_sources(
        services.context(),
        inputs.exact_toolchain_sources,
        &provenance,
    )?
    .declarations;
    Ok(SelectedPolicyProviders {
        bodies,
        declarations,
    })
}

fn is_boundary_requirement(
    typed: &checked_trees::typed_trees::TypedTrees,
    requirement: SymbolHandle,
) -> bool {
    typed
        .trait_machine_signature(requirement)
        .is_some_and(|(owner, _)| owner.is_boundary)
        || typed
            .operator_definition_by_symbol(requirement)
            .is_some_and(|operator| operator.is_boundary)
        || typed.machine(requirement).is_some_and(|machine| {
            machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
}
