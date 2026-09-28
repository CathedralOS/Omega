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
            .entry(resolution.requirement_identity.clone())
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

/// One nested requirement's resolution addressed the way
/// `SelectedProviderPlanFacts::resolve_installation_reach_for_plan` resolves
/// the root closure: the pending row's own plan's resolution first, then a
/// requirement realized by exactly one selected plan. An identity published
/// by several plans with none under this row's plan is ambiguous for this
/// row — it stays unresolved rather than binding a foreign plan's row by
/// roster order.
fn plan_scoped_resolution<'a>(
    resolutions: &'a [effects::InstallationReachResolution],
    identity_publishers: &std::collections::BTreeMap<String, std::collections::BTreeSet<u64>>,
    provider_plan_report_identity: u64,
    requirement_identity: &str,
) -> Option<&'a effects::InstallationReachResolution> {
    if let Some(own) = resolutions.iter().find(|resolution| {
        resolution.requirement_identity == requirement_identity
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
    resolutions
        .iter()
        .find(|resolution| resolution.requirement_identity == requirement_identity)
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

#[cfg(test)]
mod tests {
    use super::plan_scoped_resolution;
    use std::collections::{BTreeMap, BTreeSet};

    fn resolution(
        plan: u64,
        requirement_identity: &str,
        resolved_row: &[&str],
    ) -> effects::InstallationReachResolution {
        effects::InstallationReachResolution {
            requirement_identity: requirement_identity.to_owned(),
            provider_plan_report_identity: plan,
            upper_bound: Vec::new(),
            resolved_row: resolved_row
                .iter()
                .map(|service| (*service).to_owned())
                .collect(),
        }
    }

    fn roster(rows: &[(&str, u64)]) -> BTreeMap<String, BTreeSet<u64>> {
        let mut publishers: BTreeMap<String, BTreeSet<u64>> = BTreeMap::new();
        for (identity, plan) in rows {
            publishers
                .entry((*identity).to_owned())
                .or_default()
                .insert(*plan);
        }
        publishers
    }

    /// A requirement identity published by two selected plans binds the
    /// pending row's own plan's resolution — the pair-keyed pick, not the
    /// first row the roster happens to hold.
    #[test]
    fn shared_identity_binds_the_pending_rows_own_plan() {
        let resolutions = vec![
            resolution(0xAAAA, "InterruptEntry::enter", &["PortIo"]),
            resolution(0xBBBB, "InterruptEntry::enter", &["MachineControl"]),
        ];
        let publishers = roster(&[
            ("InterruptEntry::enter", 0xAAAA),
            ("InterruptEntry::enter", 0xBBBB),
        ]);

        assert_eq!(
            plan_scoped_resolution(&resolutions, &publishers, 0xBBBB, "InterruptEntry::enter")
                .map(|row| row.resolved_row.as_slice()),
            Some([String::from("MachineControl")].as_slice())
        );
        assert_eq!(
            plan_scoped_resolution(&resolutions, &publishers, 0xAAAA, "InterruptEntry::enter")
                .map(|row| row.resolved_row.as_slice()),
            Some([String::from("PortIo")].as_slice())
        );
    }

    /// A requirement realized by exactly one selected plan still resolves
    /// unscoped for a pending row under another plan.
    #[test]
    fn singly_published_identity_resolves_unscoped() {
        let resolutions = vec![
            resolution(0xAAAA, "InterruptAcknowledgement::complete", &["PortIo"]),
            resolution(0xBBBB, "InterruptEntry::enter", &["MachineControl"]),
        ];
        let publishers = roster(&[
            ("InterruptAcknowledgement::complete", 0xAAAA),
            ("InterruptEntry::enter", 0xBBBB),
        ]);

        assert_eq!(
            plan_scoped_resolution(
                &resolutions,
                &publishers,
                0xCCCC,
                "InterruptAcknowledgement::complete",
            )
            .map(|row| row.resolved_row.as_slice()),
            Some([String::from("PortIo")].as_slice())
        );
    }

    /// A shared identity with no row under the pending row's own plan stays
    /// unresolved rather than silently binding another plan's row.
    #[test]
    fn shared_identity_without_an_own_plan_row_stays_unresolved() {
        let resolutions = vec![
            resolution(0xAAAA, "InterruptEntry::enter", &["PortIo"]),
            resolution(0xBBBB, "InterruptEntry::enter", &["MachineControl"]),
        ];
        let publishers = roster(&[
            ("InterruptEntry::enter", 0xAAAA),
            ("InterruptEntry::enter", 0xBBBB),
        ]);

        assert!(
            plan_scoped_resolution(&resolutions, &publishers, 0xCCCC, "InterruptEntry::enter")
                .is_none()
        );
    }

    /// A row published by one plan but not yet resolved stays pending until
    /// its publisher resolves — the publisher set counts rows in flight, not
    /// just rows already emitted.
    #[test]
    fn pending_publisher_keeps_the_identity_shared() {
        // Only plan 0xBBBB's row is resolved so far; 0xAAAA's row for the
        // same identity is still pending. A foreign pending row must not
        // bind 0xBBBB's row while the identity is shared.
        let resolutions = vec![resolution(
            0xBBBB,
            "InterruptEntry::enter",
            &["MachineControl"],
        )];
        let publishers = roster(&[
            ("InterruptEntry::enter", 0xAAAA),
            ("InterruptEntry::enter", 0xBBBB),
        ]);

        assert!(
            plan_scoped_resolution(&resolutions, &publishers, 0xCCCC, "InterruptEntry::enter")
                .is_none()
        );
    }

    /// A requirement nobody publishes resolves to nothing, which leaves the
    /// pending row's nested reach unsubstituted for the rejection diagnostic.
    #[test]
    fn unpublished_identity_stays_unresolved() {
        let resolutions = vec![resolution(0xAAAA, "InterruptEntry::enter", &["PortIo"])];
        let publishers = roster(&[("InterruptEntry::enter", 0xAAAA)]);

        assert!(
            plan_scoped_resolution(&resolutions, &publishers, 0xAAAA, "Endpoint::step").is_none()
        );
    }
}
