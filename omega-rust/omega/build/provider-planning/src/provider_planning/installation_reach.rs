//! Resolve installation-bound reach from exact selected realizations.

use super::satisfied_requirement_identity;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};

pub(super) fn derive_selected_installation_reach_resolutions(
    checked: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<effects::InstallationReachResolution>, Vec<diagnostics::Diagnostic>> {
    let mut resolutions = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected.plans() {
        let top_level_requirements = checked
            .typed
            .machines()
            .iter()
            .filter(|requirement| {
                requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && crate::service_schema::from_typed_boundary_requirement(
                        &checked.typed,
                        requirement,
                    )
                    .as_ref()
                        == Some(&plan.schema)
            })
            .collect::<Vec<_>>();
        match top_level_requirements.as_slice() {
            [requirement] => {
                for row in &plan.rows {
                    append_top_level_installation_reach_resolution(
                        checked,
                        plan,
                        row,
                        requirement,
                        &mut resolutions,
                        &mut diagnostics,
                    );
                }
                continue;
            }
            [] => {}
            requirements => {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider schema `{}` resolves to {} exact top-level boundary requirements",
                    plan.schema.trait_name,
                    requirements.len(),
                )));
                continue;
            }
        }
        // Boundary operators share the provider-plan carrier, but they are
        // compiler-owned operator slots rather than boundary-trait
        // requirements. Candidate validation has already replayed their exact
        // typed operator schema. Do not make the trait-only installation-reach
        // pass reinterpret them as missing trait requirements.
        let is_boundary_operator_plan = checked.typed.operators().iter().any(|operator| {
            crate::service_schema::from_typed_operator(&checked.typed, operator).as_ref()
                == Some(&plan.schema)
        });
        if is_boundary_operator_plan {
            continue;
        }
        for row in &plan.rows {
            let requirements = checked
                .typed
                .traits()
                .iter()
                .flat_map(|owner| {
                    checked
                        .typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .filter(move |requirement| {
                            checked
                                .typed
                                .normalized_trait_requirement_overload_identity(owner, requirement)
                                .identity()
                                == row.requirement_identity
                        })
                        .map(move |requirement| (owner, requirement))
                })
                .collect::<Vec<_>>();
            let [(_, requirement)] = requirements.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact typed requirements",
                    row.requirement_identity,
                    requirements.len()
                )));
                continue;
            };
            if !requirement.service_reach_is_installation_bound {
                continue;
            }

            let realization_machines = checked
                .typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine
                        .attached_data
                        .as_ref()
                        .map(|name| name.as_str())
                        .unwrap_or_default()
                        == plan.provider_type
                })
                .filter(|machine| {
                    checked
                        .typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|conformance| {
                            conformance.requirement.as_ref().is_some_and(|name| {
                                satisfied_requirement_identity(
                                    &checked.typed,
                                    machine.name.as_str(),
                                    conformance.name.as_str(),
                                    name.as_str(),
                                ) == row.requirement_identity
                            })
                        })
                })
                .collect::<Vec<_>>();
            let [realization] = realization_machines.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact realization machines for provider `{}`",
                    row.requirement_identity,
                    realization_machines.len(),
                    plan.provider_type
                )));
                continue;
            };
            let Some(envelope) = checked
                .facts
                .contract_plans
                .realized_envelope(realization.symbol)
            else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider realization `{}` has no checked contract envelope",
                    realization.name
                )));
                continue;
            };
            if let Some(diagnostic) = unresolved_realization_reach_diagnostic(
                plan,
                &row.requirement_identity,
                realization,
                envelope,
            ) {
                diagnostics.push(diagnostic);
                continue;
            }
            let upper_bound = checked
                .facts
                .service_reaches
                .rows
                .services(requirement.service_reach_row)
                .iter()
                .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
                .map(|definition| definition.name.clone())
                .collect();
            resolutions.push(effects::InstallationReachResolution {
                requirement_identity: row.requirement_identity.clone(),
                provider_plan_report_identity: plan.report_fingerprint(),
                upper_bound,
                resolved_row: envelope.effective_service_reach.clone(),
            });
        }
    }
    if diagnostics.is_empty() {
        Ok(resolutions)
    } else {
        Err(diagnostics)
    }
}

fn append_top_level_installation_reach_resolution(
    checked: &checked_trees::CheckedTrees,
    plan: &ProviderPlan,
    row: &ProviderPlanRow,
    requirement: &typed_trees::machine::Machine,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let requirement_identity = checked
        .typed
        .normalized_machine_overload_identity(requirement)
        .map(|identity| identity.identity())
        .unwrap_or_default();
    if row.requirement_identity != requirement_identity {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected provider row `{}` does not retain exact top-level requirement `{requirement_identity}`",
            row.requirement_identity,
        )));
        return;
    }
    if !requirement.service_reach_is_installation_bound {
        return;
    }
    let realizations = checked
        .typed
        .machines()
        .iter()
        .filter(|machine| {
            machine
                .attached_data
                .as_ref()
                .is_some_and(|name| name.as_str() == plan.provider_type)
        })
        .filter(|machine| {
            checked
                .typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    conformance.symbol == requirement.symbol
                        && conformance.requirement_symbol == requirement.symbol
                        && matches!(
                            typed_trees::machine::resolve_satisfied_declaration(
                                &checked.typed,
                                machine,
                                conformance,
                            ),
                            Some(
                                typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
                                    selected,
                                ),
                            ) if selected.symbol == requirement.symbol
                        )
                })
        })
        .collect::<Vec<_>>();
    let [realization] = realizations.as_slice() else {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected top-level requirement row `{requirement_identity}` resolves to {} exact realization machines for provider `{}`",
            realizations.len(),
            plan.provider_type,
        )));
        return;
    };
    let Some(envelope) = checked
        .facts
        .contract_plans
        .realized_envelope(realization.symbol)
    else {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected provider realization `{}` has no checked contract envelope",
            realization.name,
        )));
        return;
    };
    if let Some(diagnostic) =
        unresolved_realization_reach_diagnostic(plan, &requirement_identity, realization, envelope)
    {
        diagnostics.push(diagnostic);
        return;
    }
    let upper_bound = checked
        .facts
        .service_reaches
        .rows
        .services(requirement.service_reach_row)
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.clone())
        .collect();
    resolutions.push(effects::InstallationReachResolution {
        requirement_identity,
        provider_plan_report_identity: plan.report_fingerprint(),
        upper_bound,
        resolved_row: envelope.effective_service_reach.clone(),
    });
}

/// A selected row is the exact reach of the installed realization. When the
/// realization itself still reaches through an unresolved installation-bound
/// requirement, its checked effective reach carries that requirement's
/// conservative upper bound, so publishing it as the resolved row would let a
/// bound stand in for concrete reach. Provider selection has no substitution
/// step for nested requirements; reject instead of degrading to the bound.
fn unresolved_realization_reach_diagnostic(
    plan: &ProviderPlan,
    requirement_identity: &str,
    realization: &typed_trees::machine::Machine,
    envelope: &checked_trees::RealizedMachineContractEnvelope,
) -> Option<diagnostics::Diagnostic> {
    let unresolved = envelope.unresolved_installation_reaches.len();
    (unresolved != 0).then(|| {
        diagnostics::Diagnostic::error(format!(
            "selected provider row `{requirement_identity}` realization `{}` of provider `{}` retains {unresolved} unresolved installation-bound requirement(s); its checked reach is a conservative bound, not a resolved row",
            realization.name, plan.provider_type,
        ))
    })
}
