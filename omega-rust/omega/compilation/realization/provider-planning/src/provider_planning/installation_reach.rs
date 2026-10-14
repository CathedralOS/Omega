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
    realization: &'a checked_trees::typed_trees::machine::Machine,
}

pub(super) fn derive_selected_installation_reach_resolutions(
    typed: &checked_trees::typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
    selected: &effects::SelectedProviderPlanFacts,
) -> Result<
    (
        Vec<effects::InstallationReachResolution>,
        terminal_psi::TerminalIdentityTable,
    ),
    Vec<diagnostics::Diagnostic>,
> {
    let mut resolutions = Vec::new();
    let mut pending = Vec::new();
    let mut diagnostics = Vec::new();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let provider_context =
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    for plan in selected.plans() {
        // A sealed catalog scalar declaration publishes its own concrete reach.
        // Rejoin its whole plan rather than treating it as a trait requirement.
        match crate::SealedPlacedAccessor::rejoin_plan(provider_context, plan) {
            Ok(Some(_)) => continue,
            Ok(None) => {}
            Err(error) => {
                diagnostics.push(error);
                continue;
            }
        }

        let top_level_requirements = typed
            .machines()
            .iter()
            .filter(|requirement| {
                requirement.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
                    && crate::service_schema::from_typed_boundary_requirement(
                        provider_context,
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
                        typed,
                        facts,
                        plan,
                        row,
                        requirement,
                        &mut identities,
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
                    plan.schema.identity_text(plan.schema.trait_name),
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
        let is_boundary_operator_plan = typed.operators().iter().any(|operator| {
            crate::service_schema::from_typed_operator(provider_context, operator).as_ref()
                == Some(&plan.schema)
        });
        if is_boundary_operator_plan {
            continue;
        }
        for row in &plan.rows {
            let requirements = typed
                .traits()
                .iter()
                .filter(|owner| crate::service_schema::is_product_declaration(typed, owner.symbol))
                .flat_map(|owner| {
                    typed
                        .trait_machine_signatures(owner)
                        .iter()
                        .filter(move |requirement| {
                            typed.trait_requirement_callable_identity_text(owner, requirement)
                                == plan.identities.text(row.requirement_identity)
                        })
                        .map(move |requirement| (owner, requirement))
                })
                .collect::<Vec<_>>();
            let [(_, requirement)] = requirements.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact typed requirements",
                    plan.identities.text(row.requirement_identity),
                    requirements.len()
                )));
                continue;
            };
            if !requirement.service_reach_is_installation_bound {
                continue;
            }

            // A named implementation's plan installs the machine its own row
            // names. An exact requirement edge's plan is provided by the data
            // owning the machine that carries that edge.
            let named_realization =
                crate::provider_identity::named_provider_row(provider_context, plan, row)
                    .and_then(|entry| {
                        crate::target_machines::realized_row_machine(
                            provider_context,
                            entry.realization_machine,
                            Some(&plan.target),
                        )
                    })
                    .map(|machine| machine.symbol);
            let is_named_provider =
                crate::provider_identity::named_provider(provider_context, plan).is_some();
            let realization_machines = typed
                .machines()
                .iter()
                .filter(|machine| {
                    crate::service_schema::is_product_declaration(typed, machine.symbol)
                })
                .filter(|machine| {
                    if is_named_provider {
                        return named_realization == Some(machine.symbol);
                    }
                    typed.attached_data_path(machine).unwrap_or_default() == plan.provider_type
                        && typed
                            .machine_trait_conformances(machine)
                            .iter()
                            .any(|conformance| {
                                conformance.requirement.is_some() && {
                                    exact_satisfied_requirement_identity(
                                        typed,
                                        conformance.symbol,
                                        conformance.requirement_symbol,
                                    ) == plan.identities.text(row.requirement_identity)
                                }
                            })
                })
                .collect::<Vec<_>>();
            let [realization] = realization_machines.as_slice() else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider row `{}` resolves to {} exact realization machines for provider `{}`",
                    plan.identities.text(row.requirement_identity),
                    realization_machines.len(),
                    plan.provider_type
                )));
                continue;
            };
            let Some(envelope) = facts.contract_plans.realized_envelope(realization.symbol) else {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
                    "selected provider realization `{}` has no checked contract envelope",
                    realization.name
                )));
                continue;
            };
            let upper_bound = service_upper_bound_names(facts, requirement.service_reach_row);
            record_installation_reach_row(
                plan,
                realization,
                envelope,
                plan.identities.text(row.requirement_identity).to_owned(),
                upper_bound,
                &mut identities,
                &mut resolutions,
                &mut pending,
            );
        }
    }
    substitute_nested_installation_reaches(
        &mut identities,
        typed,
        &mut resolutions,
        pending,
        &mut diagnostics,
    );
    if diagnostics.is_empty() {
        Ok((resolutions, identities.build()))
    } else {
        Err(diagnostics)
    }
}

fn append_top_level_installation_reach_resolution<'a>(
    typed: &'a checked_trees::typed_trees::TypedTrees,
    facts: &'a checked_trees::CheckFacts,
    plan: &'a ProviderPlan,
    row: &ProviderPlanRow,
    requirement: &'a checked_trees::typed_trees::machine::Machine,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    pending: &mut Vec<PendingInstallationReachRow<'a>>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    let requirement_identity = typed
        .machine_callable_identity_text(requirement)
        .unwrap_or_default();
    if plan.identities.text(row.requirement_identity) != requirement_identity.as_str() {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected provider row `{}` does not retain exact top-level requirement `{requirement_identity}`",
            plan.identities.text(row.requirement_identity),
        )));
        return;
    }
    if !requirement.service_reach_is_installation_bound {
        return;
    }
    let realizations = typed
        .machines()
        .iter()
        .filter(|machine| {
            typed.attached_data_path(machine).as_deref() == Some(plan.provider_type.as_str())
        })
        .filter(|machine| {
            typed
                .machine_trait_conformances(machine)
                .iter()
                .any(|conformance| {
                    conformance.symbol == requirement.symbol
                        && conformance.requirement_symbol == requirement.symbol
                        && matches!(
                            checked_trees::typed_trees::machine::resolve_satisfied_declaration(
                                typed,
                                machine,
                                conformance,
                            ),
                            Some(
                                checked_trees::typed_trees::machine::SatisfiedDeclaration::TopLevelRequirement(
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
    let Some(envelope) = facts.contract_plans.realized_envelope(realization.symbol) else {
        diagnostics.push(diagnostics::Diagnostic::error(format!(
            "selected provider realization `{}` has no checked contract envelope",
            realization.name,
        )));
        return;
    };
    let upper_bound = service_upper_bound_names(facts, requirement.service_reach_row);
    record_installation_reach_row(
        plan,
        realization,
        envelope,
        requirement_identity,
        upper_bound,
        identities,
        resolutions,
        pending,
    );
}

/// The reach requirement's declared service-name ceiling: the resolved
/// names of every service definition its `service_reach_row` admits.
fn service_upper_bound_names(
    facts: &checked_trees::CheckFacts,
    service_reach_row: language_semantics::ServiceReachRowId,
) -> Vec<String> {
    facts
        .service_reaches
        .rows
        .services(service_reach_row)
        .iter()
        .filter_map(|service| facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.clone())
        .collect()
}

fn record_installation_reach_row<'a>(
    plan: &'a ProviderPlan,
    realization: &'a checked_trees::typed_trees::machine::Machine,
    envelope: &'a checked_trees::RealizedMachineContractEnvelope,
    requirement_identity: String,
    upper_bound: Vec<String>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    pending: &mut Vec<PendingInstallationReachRow<'a>>,
) {
    if envelope.unresolved_installation_reaches.is_empty() {
        resolutions.push(effects::InstallationReachResolution {
            requirement_identity: identities.intern(&requirement_identity),
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    typed: &checked_trees::typed_trees::TypedTrees,
    resolutions: &mut Vec<effects::InstallationReachResolution>,
    mut pending: Vec<PendingInstallationReachRow<'_>>,
    diagnostics: &mut Vec<diagnostics::Diagnostic>,
) {
    // Every selected plan publishes one row per requirement identity, so the
    // publisher set for an identity is resolutions plus rows still pending —
    // counting only resolved rows would misread a shared identity as
    // unambiguous while a second plan's row is still in flight.
    let mut identity_publishers: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<u64>,
    > = std::collections::BTreeMap::new();
    for resolution in resolutions.iter() {
        identity_publishers
            .entry(
                identities
                    .text(resolution.requirement_identity)
                    .unwrap_or_default()
                    .to_owned(),
            )
            .or_default()
            .insert(resolution.provider_plan_report_identity);
    }
    for row in &pending {
        identity_publishers
            .entry(row.requirement_identity.clone())
            .or_default()
            .insert(row.provider_plan_report_identity);
    }
    loop {
        let mut next = Vec::new();
        let mut progressed = false;
        for mut row in pending.drain(..) {
            let mut resolved_row = row.concrete_service_reach.clone();
            row.unresolved.retain(|reach| {
                let resolved =
                    nested_requirement_identity(typed, reach.requirement).and_then(|identity| {
                        plan_scoped_resolution(
                            resolutions,
                            &identities,
                            &identity_publishers,
                            row.provider_plan_report_identity,
                            &identity,
                        )
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
                    requirement_identity: identities.intern(&row.requirement_identity),
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

/// One nested requirement's resolution addressed the way
/// `SelectedProviderPlanFacts::resolve_installation_reach_for_plan` resolves
/// the root closure: the pending row's own plan's resolution first, then a
/// requirement realized by exactly one selected plan. An identity published
/// by several plans with none under this row's plan is ambiguous for this
/// row — it stays unresolved rather than binding a foreign plan's row by
/// roster order.
fn plan_scoped_resolution<'a>(
    resolutions: &'a [effects::InstallationReachResolution],
    identities: &terminal_psi::TerminalIdentityTableBuilder,
    identity_publishers: &std::collections::BTreeMap<String, std::collections::BTreeSet<u64>>,
    provider_plan_report_identity: u64,
    requirement_identity: &str,
) -> Option<&'a effects::InstallationReachResolution> {
    if let Some(own) = resolutions.iter().find(|resolution| {
        identities.text(resolution.requirement_identity) == Some(requirement_identity)
            && resolution.provider_plan_report_identity == provider_plan_report_identity
    }) {
        return Some(own);
    }
    if identity_publishers
        .get(requirement_identity)
        .is_some_and(|publishers| publishers.len() > 1)
    {
        return None;
    }
    resolutions.iter().find(|resolution| {
        identities.text(resolution.requirement_identity) == Some(requirement_identity)
    })
}

/// The requirement identity a nested installation-bound row refers to: a
/// top-level requirement machine resolves to its own overload identity and a
/// trait requirement resolves to its exact overload identity under the trait
/// definition that declares it.
fn nested_requirement_identity(
    typed: &checked_trees::typed_trees::TypedTrees,
    requirement: symbols::SymbolHandle,
) -> Option<String> {
    if let Some(machine) = typed.machines().iter().find(|machine| {
        machine.symbol == requirement
            && machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
    }) {
        return typed
            .machine_callable_identity_text(machine)
            .filter(|identity| !identity.is_empty());
    }
    typed.traits().iter().find_map(|definition| {
        typed
            .trait_machine_signature_by_symbol(definition, requirement)
            .and_then(|signature| {
                let identity =
                    typed.trait_requirement_callable_identity_text(definition, signature);
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
    realization: &checked_trees::typed_trees::machine::Machine,
    unresolved: usize,
) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(format!(
        "selected provider row `{requirement_identity}` realization `{}` of provider `{}` retains {unresolved} unresolved installation-bound requirement(s); its checked reach is a conservative bound, not a resolved row",
        realization.name, plan.provider_type,
    ))
}
