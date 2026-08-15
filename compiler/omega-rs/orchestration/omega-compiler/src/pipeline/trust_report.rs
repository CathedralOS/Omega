//! GR5/GR6 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment, plus exact provider-requirement and routed
//! qualification rows copied from normalized provider plans. Domain
//! introductions, accepted facts, provider plans, and their requirement blast
//! radius retain root-grant or dev-active provenance; the latter carries a
//! standing warning.

use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, TrustProviderRealization, TrustProviderRequirementRow, TrustQualificationRow,
    TrustReport, TrustReportRow,
};
use psi_diagnostics::Diagnostic;

pub(super) fn write_trust_report(
    options: &CompileOptions,
    checked: &psi_checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let typed = &checked.typed;
    let mut report = TrustReport::default();
    report.selected_provider_closure_fingerprint = selected_provider_plans.normalized_identity();
    let mut recognized_provider_grants = Vec::new();
    // PRV3: derived provider plans -- one row each, dev-active with the
    // standing warning until the final build grants the plan by name (or
    // its trait leaf), fingerprint shown so drift is visible at a glance.
    for plan in provider_plans {
        let leaf = plan.schema.trait_name.as_str();
        let selected = selected_provider_plans
            .plan_by_identity(plan.identity_fingerprint())
            .is_some();
        let grant_selectors = selected
            .then(|| {
                root_grants
                    .iter()
                    .filter(|grant| *grant == &plan.name || *grant == leaf)
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for selector in &grant_selectors {
            if !recognized_provider_grants.contains(selector) {
                recognized_provider_grants.push(selector.clone());
            }
        }
        let granted = !grant_selectors.is_empty();
        let provenance = if granted {
            "root grant (build.omg)"
        } else {
            "own-package (dev-active)"
        };
        let covered = plan
            .schema
            .methods
            .iter()
            .filter(|method| {
                plan.rows
                    .iter()
                    .any(|row| plan.schema.row_binds_method(row, method))
            })
            .count();
        report.rows.push(TrustReportRow {
            commitment: format!(
                "provider plan: {} [{:016x}] provider type: {} target: {} coverage {covered}/{} selected: {}",
                plan.name,
                plan.identity_fingerprint(),
                if plan.provider_type.is_empty() {
                    "<free external>"
                } else {
                    plan.provider_type.as_str()
                },
                if plan.target.is_empty() {
                    "<all>"
                } else {
                    plan.target.as_str()
                },
                plan.schema.methods.len(),
                if selected { "yes" } else { "no" },
            ),
            provenance: provenance.to_owned(),
            machine_contract_fingerprint: None,
            machine_service_reach: None,
            machine_synchronous_invocations: None,
            machine_may_suspend: None,
            standing_warning: !granted,
        });
        let mut bound_methods = Vec::with_capacity(plan.rows.len());
        for row in &plan.rows {
            let (method_index, method) = plan
                .schema
                .methods
                .iter()
                .enumerate()
                .find(|(_, method)| plan.schema.row_binds_method(row, method))
                .expect("validated provider rows bind one exact schema requirement");
            bound_methods.push((method_index, method));
            report
                .provider_requirements
                .push(TrustProviderRequirementRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    provider_type: plan.provider_type.clone(),
                    target: plan.target.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    calling_plan_fingerprint: method.calling_plan_fingerprint,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_identity: row.requirement_identity.clone(),
                    method: row.method.clone(),
                    parameter_type_identities: method.parameter_type_identities.clone(),
                    result_type_identity: method.result_type_identity.clone(),
                    service_reach: method.service_reach.clone(),
                    synchronous_invocations: method.synchronous_invocations.clone(),
                    may_suspend: method.may_suspend,
                    may_block: method.may_block,
                    terminates_guarantee: method.terminates_guarantee,
                    realization: trust_provider_realization(&row.binding),
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
                    standing_warning: !granted,
                });
        }
        // Preserve schema declaration order while excluding every unbound
        // requirement from a partial candidate's qualification blast radius.
        bound_methods.sort_unstable_by_key(|(method_index, _)| *method_index);
        for (_, method) in bound_methods {
            for claim in &method.entry_claims {
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    provider_type: plan.provider_type.clone(),
                    target: plan.target.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    calling_plan_fingerprint: method.calling_plan_fingerprint,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_identity: method.requirement_identity.clone(),
                    method: method.name.clone(),
                    subject: format!("parameter:{}", claim.parameter_index),
                    authority_flow: claim.authority_flow.as_str().to_owned(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry.to_string(),
                    predicate_discharge_required: claim.predicate_body.is_present(),
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
                    standing_warning: !granted,
                });
            }
            for claim in &method.result_claims {
                // ServiceResultClaim contains only bodyless routed results;
                // predicate-bearing establishment is deliberately absent from
                // this generic provider-result carrier.
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_fingerprint: plan.identity_fingerprint(),
                    provider_type: plan.provider_type.clone(),
                    target: plan.target.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    calling_plan_fingerprint: method.calling_plan_fingerprint,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_identity: method.requirement_identity.clone(),
                    method: method.name.clone(),
                    subject: "result".to_owned(),
                    authority_flow: "returns".to_owned(),
                    domain: claim.domain.clone(),
                    effective_carry: claim.effective_carry.to_string(),
                    predicate_discharge_required: false,
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
                    standing_warning: !granted,
                });
            }
        }
    }
    for domain in typed.domain_definitions() {
        if !domain.semantic_id.is_valid() {
            continue;
        }
        // A root grant naming this domain (by full rendered name or leaf)
        // flips its provenance and retires the standing warning (GR3).
        let leaf = domain
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(domain.name.as_str());
        let granted = root_grants
            .iter()
            .any(|grant| grant == domain.name.as_str() || grant == leaf);
        report.rows.push(TrustReportRow {
            commitment: format!("domain introduction: {}", domain.name.as_str()),
            provenance: if granted {
                "root grant (build.omg)".to_owned()
            } else {
                "own-package (dev-active)".to_owned()
            },
            machine_contract_fingerprint: None,
            machine_service_reach: None,
            machine_synchronous_invocations: None,
            machine_may_suspend: None,
            standing_warning: !granted,
        });
    }
    // ACCEPTED machines (bodyless boundary axioms, GR6d): one row each --
    // own-package dev-active with the standing warning, or root-granted
    // when build.omg names the machine.
    for machine in typed.machines() {
        if machine.supply_mode != psi_language_semantics::MachineSupplyMode::Accepted {
            continue;
        }
        let leaf = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(machine.name.as_str());
        let granted = root_grants
            .iter()
            .any(|grant| grant == machine.name.as_str() || grant == leaf);
        let machine_contract_fingerprint = checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "accepted machine `{}` has no exact checked contract plan",
                    machine.name.as_str()
                ))]
            })?
            .fingerprint;
        let machine_service_reach =
            accepted_machine_service_reach(checked, machine.symbol, machine.name.as_str())
                .map_err(|diagnostic| vec![diagnostic])?;
        let machine_synchronous_invocations = accepted_machine_synchronous_invocations(
            checked,
            machine.symbol,
            machine.name.as_str(),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let machine_may_suspend =
            accepted_machine_may_suspend(checked, machine.symbol, machine.name.as_str())
                .map_err(|diagnostic| vec![diagnostic])?;
        report.rows.push(TrustReportRow {
            commitment: format!("accepted fact: {}", machine.name.as_str()),
            provenance: if granted {
                "root grant (build.omg)".to_owned()
            } else {
                "own-package (dev-active)".to_owned()
            },
            machine_contract_fingerprint: Some(machine_contract_fingerprint),
            machine_service_reach: Some(machine_service_reach),
            machine_synchronous_invocations: Some(machine_synchronous_invocations),
            machine_may_suspend: Some(machine_may_suspend),
            standing_warning: !granted,
        });
    }
    // Grants naming anything other than a declared domain, an accepted
    // machine, or an already-reported selected provider plan surface as bare
    // accepted-fact rows (the report shows every grant, private or public).
    for grant in root_grants {
        let names_domain = typed.domain_definitions().iter().any(|domain| {
            grant == domain.name.as_str()
                || Some(grant.as_str()) == domain.name.as_str().rsplit("::").next()
        });
        let names_accepted = typed.machines().iter().any(|machine| {
            machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
                && (grant == machine.name.as_str()
                    || Some(grant.as_str()) == machine.name.as_str().rsplit("::").next())
        });
        let names_selected_provider = recognized_provider_grants.contains(grant);
        if !names_domain && !names_accepted && !names_selected_provider {
            report.rows.push(TrustReportRow {
                commitment: format!("accepted fact: {grant}"),
                provenance: "root grant (build.omg)".to_owned(),
                machine_contract_fingerprint: None,
                machine_service_reach: None,
                machine_synchronous_invocations: None,
                machine_may_suspend: None,
                standing_warning: false,
            });
        }
    }

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_trust_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}

fn accepted_machine_service_reach(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<Vec<String>, Diagnostic> {
    let machine_reach = checked
        .facts
        .service_reaches
        .for_machine(machine)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked service-reach facts"
            ))
        })?;
    let psi_language_semantics::ServiceReachInterface::PublishedCeiling(reach_row) =
        machine_reach.interface
    else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published service-reach ceiling"
        )));
    };
    checked
        .facts
        .service_reaches
        .rows
        .services(reach_row)
        .iter()
        .map(|service| {
            checked
                .facts
                .service_reaches
                .services
                .definition(*service)
                .map(|definition| definition.name.clone())
                .ok_or_else(|| {
                    Diagnostic::error(format!(
                        "accepted machine `{machine_name}` references an unknown service-reach identity"
                    ))
                })
        })
        .collect()
}

fn accepted_machine_synchronous_invocations(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<Vec<String>, Diagnostic> {
    let plan = checked
        .facts
        .synchronous_invocations
        .for_machine(machine)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked synchronous-invocation facts"
            ))
        })?;
    if plan.interface != psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published synchronous-invocation ceiling"
        )));
    }
    Ok(plan.published.clone())
}

fn accepted_machine_may_suspend(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<bool, Diagnostic> {
    let plan = checked
        .facts
        .suspensions
        .for_machine(machine)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked suspension facts"
            ))
        })?;
    let psi_language_semantics::SuspensionInterface::PublishedMaySuspend(may_suspend) =
        plan.interface
    else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published suspension ceiling"
        )));
    };
    Ok(may_suspend)
}

fn trust_provider_realization(
    binding: &omega_effects::provider_plan::ProviderBinding,
) -> TrustProviderRealization {
    use omega_effects::provider_plan::ProviderBinding;

    match binding {
        ProviderBinding::Import { library, symbol } => TrustProviderRealization::Import {
            library: library.clone(),
            symbol: symbol.clone(),
        },
        ProviderBinding::Syscall { number } => {
            TrustProviderRealization::Syscall { number: *number }
        }
        ProviderBinding::CompilerIntrinsic { name } => {
            TrustProviderRealization::CompilerIntrinsic { name: name.clone() }
        }
        ProviderBinding::VtableSlot { index } => {
            TrustProviderRealization::VtableSlot { index: *index }
        }
        ProviderBinding::VtableField { table, field } => TrustProviderRealization::VtableField {
            table: table.clone(),
            field: field.clone(),
        },
        ProviderBinding::TableFunction { table, field } => {
            TrustProviderRealization::TableFunction {
                table: table.clone(),
                field: field.clone(),
            }
        }
        ProviderBinding::CheckedAdapter { machine } => TrustProviderRealization::CheckedAdapter {
            machine: machine.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use psi_checked_trees::{
        CheckedTrees, MachineServiceReachRows, MachineSuspensionFact,
        MachineSynchronousInvocationFact, ServiceReachFacts,
    };
    use psi_language_semantics::{
        ServiceReachInterface, ServiceReachRowTable, ServiceReachTable, SuspensionInterface,
        SuspensionPlan, SynchronousInvocationInterface, SynchronousInvocationPlan,
    };
    use psi_symbols::SymbolHandle;

    use super::{
        accepted_machine_may_suspend, accepted_machine_service_reach,
        accepted_machine_synchronous_invocations,
    };

    fn checked_with_reach(
        machine: SymbolHandle,
        interface: ServiceReachInterface,
        services: ServiceReachTable,
        rows: ServiceReachRowTable,
    ) -> CheckedTrees {
        let mut service_reaches = ServiceReachFacts {
            services,
            rows,
            ..Default::default()
        };
        service_reaches.machines.append_to_span(
            &mut service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                interface,
                ..Default::default()
            },
        );
        let mut checked = CheckedTrees::default();
        checked.facts.service_reaches = service_reaches;
        checked
    }

    #[test]
    fn accepted_service_reach_projects_only_exact_published_registry_rows() {
        let machine = SymbolHandle::from_arena_index(1);
        let service_symbol = SymbolHandle::from_arena_index(2);
        let mut services = ServiceReachTable::default();
        let service = services.intern(service_symbol, "Clock");
        let mut rows = ServiceReachRowTable::default();
        let row = rows.intern(vec![service]);
        let checked = checked_with_reach(
            machine,
            ServiceReachInterface::PublishedCeiling(row),
            services,
            rows,
        );

        assert_eq!(
            accepted_machine_service_reach(&checked, machine, "accepted"),
            Ok(vec!["Clock".to_owned()])
        );
    }

    #[test]
    fn accepted_service_reach_fails_closed_on_missing_internal_and_unknown_facts() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing = accepted_machine_service_reach(&CheckedTrees::default(), machine, "missing")
            .expect_err("missing facts reject")
            .to_string();
        assert!(missing.contains("has no exact checked service-reach facts"));

        let internal = checked_with_reach(
            machine,
            ServiceReachInterface::InternalInferred,
            ServiceReachTable::default(),
            ServiceReachRowTable::default(),
        );
        let internal = accepted_machine_service_reach(&internal, machine, "internal")
            .expect_err("private inference rejects")
            .to_string();
        assert!(internal.contains("has no published service-reach ceiling"));

        let mut foreign_services = ServiceReachTable::default();
        let unknown_service = foreign_services.intern(SymbolHandle::from_arena_index(2), "Unknown");
        let mut rows = ServiceReachRowTable::default();
        let row = rows.intern(vec![unknown_service]);
        let unknown = checked_with_reach(
            machine,
            ServiceReachInterface::PublishedCeiling(row),
            ServiceReachTable::default(),
            rows,
        );
        let unknown = accepted_machine_service_reach(&unknown, machine, "unknown")
            .expect_err("unregistered service rejects")
            .to_string();
        assert!(unknown.contains("references an unknown service-reach identity"));
    }

    #[test]
    fn accepted_synchronous_invocations_copy_only_the_exact_published_vector() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut checked = CheckedTrees::default();
        checked
            .facts
            .synchronous_invocations
            .machines
            .push(MachineSynchronousInvocationFact {
                machine,
                plan: SynchronousInvocationPlan {
                    interface: SynchronousInvocationInterface::PublishedCeiling,
                    published: vec!["parameter:0".to_owned(), "service:Clock".to_owned()],
                    checked_inferred: vec!["service:Private".to_owned()],
                },
            });

        assert_eq!(
            accepted_machine_synchronous_invocations(&checked, machine, "accepted"),
            Ok(vec!["parameter:0".to_owned(), "service:Clock".to_owned()])
        );
    }

    #[test]
    fn accepted_synchronous_invocations_fail_closed_on_missing_and_internal_facts() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing =
            accepted_machine_synchronous_invocations(&CheckedTrees::default(), machine, "missing")
                .expect_err("missing facts reject")
                .to_string();
        assert!(missing.contains("has no exact checked synchronous-invocation facts"));

        let mut internal = CheckedTrees::default();
        internal
            .facts
            .synchronous_invocations
            .machines
            .push(MachineSynchronousInvocationFact {
                machine,
                plan: SynchronousInvocationPlan {
                    interface: SynchronousInvocationInterface::InternalInferred,
                    published: Vec::new(),
                    checked_inferred: vec!["parameter:0".to_owned()],
                },
            });
        let internal = accepted_machine_synchronous_invocations(&internal, machine, "internal")
            .expect_err("private inference rejects")
            .to_string();
        assert!(internal.contains("has no published synchronous-invocation ceiling"));
    }

    #[test]
    fn accepted_suspension_copies_only_the_published_interface_bit() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut checked = CheckedTrees::default();
        checked
            .facts
            .suspensions
            .machines
            .push(MachineSuspensionFact {
                machine,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::PublishedMaySuspend(true),
                    checked_may_suspend: false,
                },
            });

        assert_eq!(
            accepted_machine_may_suspend(&checked, machine, "accepted"),
            Ok(true)
        );
    }

    #[test]
    fn accepted_suspension_fails_closed_on_missing_and_internal_facts() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing = accepted_machine_may_suspend(&CheckedTrees::default(), machine, "missing")
            .expect_err("missing facts reject")
            .to_string();
        assert!(missing.contains("has no exact checked suspension facts"));

        let mut internal = CheckedTrees::default();
        internal
            .facts
            .suspensions
            .machines
            .push(MachineSuspensionFact {
                machine,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::InternalInferred,
                    checked_may_suspend: true,
                },
            });
        let internal = accepted_machine_may_suspend(&internal, machine, "internal")
            .expect_err("private inference rejects")
            .to_string();
        assert!(internal.contains("has no published suspension ceiling"));
    }
}
