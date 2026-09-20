//! Resolve installation-bound reach from exact selected realizations.

use super::provenance_replay::exact_satisfied_requirement_identity;
use effects::provider_plan::{ProviderPlan, ProviderPlanRow};

/// A selected row whose realization still reaches through unresolved
/// installation-bound requirements. Its nested rows substitute against the
/// resolutions derived for the same closure before it can publish one.
struct PendingInstallationReachRow<'a> {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    upper_bound: Vec<String>,
    concrete_service_reach: Vec<String>,
    unresolved: Vec<flow_effects::InstallationReachRequirement>,
    plan: &'a ProviderPlan,
    realization: &'a typed_trees::machine::Machine,
}

pub(super) fn derive_selected_installation_reach_resolutions(
    checked: &checked_trees::CheckedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<Vec<effects::InstallationReachResolution>, Vec<diagnostics::Diagnostic>> {
    let mut resolutions = Vec::new();
    let mut pending = Vec::new();
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
                        &mut pending,
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
                .filter(|owner| {
                    crate::service_schema::is_product_declaration(&checked.typed, owner.symbol)
                })
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
                    crate::service_schema::is_product_declaration(&checked.typed, machine.symbol)
                })
                .filter(|machine| {
                    checked
                        .typed
                        .attached_data_path(machine)
                        .unwrap_or_default()
                        == plan.provider_type
                })
                .filter(|machine| {
                    checked
                        .typed
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|conformance| {
                            conformance.requirement.is_some() && {
                                exact_satisfied_requirement_identity(
                                    &checked.typed,
                                    conformance.symbol,
                                    conformance.requirement_symbol,
                                ) == row.requirement_identity
                            }
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
            let upper_bound = checked
                .facts
                .service_reaches
                .rows
                .services(requirement.service_reach_row)
                .iter()
                .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
                .map(|definition| definition.name.clone())
                .collect();
            record_installation_reach_row(
                plan,
                realization,
                envelope,
                row.requirement_identity.clone(),
                upper_bound,
                &mut resolutions,
                &mut pending,
            );
        }
    }
    substitute_nested_installation_reaches(
        &checked.typed,
        &mut resolutions,
        pending,
        &mut diagnostics,
    );
    if diagnostics.is_empty() {
        Ok(resolutions)
    } else {
        Err(diagnostics)
    }
}

fn append_top_level_installation_reach_resolution<'a>(
    checked: &'a checked_trees::CheckedTrees,
    plan: &'a ProviderPlan,
    row: &ProviderPlanRow,
    requirement: &'a typed_trees::machine::Machine,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    pending: &mut Vec<PendingInstallationReachRow<'a>>,
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
            checked.typed.attached_data_path(machine).as_deref()
                == Some(plan.provider_type.as_str())
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
    let upper_bound = checked
        .facts
        .service_reaches
        .rows
        .services(requirement.service_reach_row)
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.clone())
        .collect();
    record_installation_reach_row(
        plan,
        realization,
        envelope,
        requirement_identity,
        upper_bound,
        resolutions,
        pending,
    );
}

fn record_installation_reach_row<'a>(
    plan: &'a ProviderPlan,
    realization: &'a typed_trees::machine::Machine,
    envelope: &'a checked_trees::RealizedMachineContractEnvelope,
    requirement_identity: String,
    upper_bound: Vec<String>,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    pending: &mut Vec<PendingInstallationReachRow<'a>>,
) {
    if envelope.unresolved_installation_reaches.is_empty() {
        resolutions.push(effects::InstallationReachResolution {
            requirement_identity,
            provider_plan_report_identity: plan.report_fingerprint(),
            upper_bound,
            resolved_row: envelope.effective_service_reach.clone(),
        });
    } else {
        pending.push(PendingInstallationReachRow {
            requirement_identity,
            provider_plan_report_identity: plan.report_fingerprint(),
            upper_bound,
            concrete_service_reach: envelope.concrete_service_reach.clone(),
            unresolved: envelope.unresolved_installation_reaches.clone(),
            plan,
            realization,
        });
    }
}

/// The installation closure substitutes every bounded row through the
/// complete root closure: a nested requirement selected inside the same
/// closure is in scope rather than excluded, and the realization's resolved
/// row is its concrete reach extended by each nested requirement's resolved
/// row. Substituted rows can themselves satisfy rows still pending, so the
/// pass iterates to a fixpoint. A row whose nested requirements have no
/// selected resolution in the closure remains unresolved and rejects.
fn substitute_nested_installation_reaches(
    typed: &typed_trees::TypedTrees,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    mut pending: Vec<PendingInstallationReachRow<'_>>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    loop {
        let mut next = Vec::new();
        let mut progressed = false;
        for mut row in pending.drain(..) {
            let mut resolved_row = row.concrete_service_reach.clone();
            row.unresolved.retain(|reach| {
                let resolved =
                    nested_requirement_identity(typed, reach.requirement).and_then(|identity| {
                        resolutions
                            .iter()
                            .find(|resolution| resolution.requirement_identity == identity)
                    });
                match resolved {
                    Some(child) => {
                        resolved_row.extend(child.resolved_row.iter().cloned());
                        false
                    }
                    None => true,
                }
            });
            if row.unresolved.is_empty() {
                resolved_row.sort();
                resolved_row.dedup();
                resolutions.push(effects::InstallationReachResolution {
                    requirement_identity: row.requirement_identity,
                    provider_plan_report_identity: row.provider_plan_report_identity,
                    upper_bound: row.upper_bound,
                    resolved_row,
                });
                progressed = true;
            } else {
                next.push(row);
            }
        }
        pending = next;
        if !progressed {
            break;
        }
    }
    for row in pending {
        diagnostics.push(unresolved_realization_reach_diagnostic(
            row.plan,
            &row.requirement_identity,
            row.realization,
            row.unresolved.len(),
        ));
    }
}

/// The requirement identity a nested installation-bound row refers to: a
/// top-level requirement machine resolves to its own overload identity and a
/// trait requirement resolves to its exact overload identity under the trait
/// definition that declares it.
fn nested_requirement_identity(
    typed: &typed_trees::TypedTrees,
    requirement: symbols::SymbolHandle,
) -> Option<String> {
    if let Some(machine) = typed.machines().iter().find(|machine| {
        machine.symbol == requirement
            && machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
    }) {
        return typed
            .normalized_machine_overload_identity(machine)
            .map(|identity| identity.identity())
            .filter(|identity| !identity.is_empty());
    }
    typed.traits().iter().find_map(|definition| {
        typed
            .trait_machine_signatures(definition)
            .iter()
            .find(|signature| signature.symbol == requirement)
            .and_then(|signature| {
                let identity = typed
                    .normalized_trait_requirement_overload_identity(definition, signature)
                    .identity();
                (!identity.is_empty()).then_some(identity)
            })
    })
}

/// A selected row is the exact reach of the installed realization. When the
/// realization still reaches through an installation-bound requirement that
/// the closure did not select, publishing the checked effective reach would
/// let that requirement's conservative upper bound stand in for concrete
/// reach; reject instead of degrading to the bound.
fn unresolved_realization_reach_diagnostic(
    plan: &ProviderPlan,
    requirement_identity: &str,
    realization: &typed_trees::machine::Machine,
    unresolved: usize,
) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(format!(
        "selected provider row `{requirement_identity}` realization `{}` of provider `{}` retains {unresolved} unresolved installation-bound requirement(s); its checked reach is a conservative bound, not a resolved row",
        realization.name, plan.provider_type,
    ))
}
