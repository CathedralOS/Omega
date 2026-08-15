//! GR5/GR6 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment, plus exact provider-requirement and routed
//! qualification rows copied from normalized provider plans. Domain
//! introductions, accepted facts, provider plans, and their requirement blast
//! radius retain root-grant or dev-active provenance; the latter carries a
//! standing warning.

use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{
    ArtifactWriter, TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard,
    TrustProviderRealization, TrustProviderRequirementRow, TrustQualificationRow, TrustReport,
    TrustReportRow,
};
use psi_diagnostics::Diagnostic;

pub(super) struct GenericAcceptedTemplateFingerprints {
    rows: Vec<GenericAcceptedTemplateFingerprint>,
}

struct GenericAcceptedTemplateFingerprint {
    machine: psi_symbols::SymbolHandle,
    fingerprint: Option<u64>,
}

impl GenericAcceptedTemplateFingerprints {
    pub(super) fn capture(typed: &psi_typed_trees::TypedTrees) -> Self {
        Self {
            rows: typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
                })
                .map(|machine| GenericAcceptedTemplateFingerprint {
                    machine: machine.symbol,
                    fingerprint:
                        psi_typed_trees_to_checked_trees::generic_machine_template_fingerprint(
                            typed,
                            machine.symbol,
                        ),
                })
                .collect(),
        }
    }

    fn for_machine(
        &self,
        machine: psi_symbols::SymbolHandle,
        machine_name: &str,
    ) -> Result<Option<u64>, Diagnostic> {
        let mut matches = self.rows.iter().filter(|row| row.machine == machine);
        let row = matches.next().ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no pre-lowering template classification"
            ))
        })?;
        if matches.next().is_some() {
            return Err(Diagnostic::error(format!(
                "accepted machine `{machine_name}` has duplicate pre-lowering template classifications"
            )));
        }
        Ok(row.fingerprint)
    }
}

pub(super) fn write_trust_report(
    options: &CompileOptions,
    checked: &psi_checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    generic_accepted_template_fingerprints: &GenericAcceptedTemplateFingerprints,
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
            machine_template_fingerprint: None,
            machine_service_reach: None,
            machine_synchronous_invocations: None,
            machine_may_suspend: None,
            machine_may_block: None,
            machine_terminates_guarantee: None,
            machine_crash_routes: None,
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
            machine_template_fingerprint: None,
            machine_service_reach: None,
            machine_synchronous_invocations: None,
            machine_may_suspend: None,
            machine_may_block: None,
            machine_terminates_guarantee: None,
            machine_crash_routes: None,
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
        let machine_template_fingerprint = generic_accepted_template_fingerprints
            .for_machine(machine.symbol, machine.name.as_str())
            .map_err(|diagnostic| vec![diagnostic])?;
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
        let machine_may_block =
            accepted_machine_may_block(checked, machine.symbol, machine.name.as_str())
                .map_err(|diagnostic| vec![diagnostic])?;
        let machine_terminates_guarantee =
            accepted_machine_terminates_guarantee(checked, machine.symbol, machine.name.as_str())
                .map_err(|diagnostic| vec![diagnostic])?;
        let machine_crash_routes =
            accepted_machine_crash_routes(checked, machine.symbol, machine.name.as_str())
                .map_err(|diagnostic| vec![diagnostic])?;
        report.rows.push(TrustReportRow {
            commitment: format!("accepted fact: {}", machine.name.as_str()),
            provenance: if granted {
                "root grant (build.omg)".to_owned()
            } else {
                "own-package (dev-active)".to_owned()
            },
            machine_contract_fingerprint: Some(machine_contract_fingerprint),
            machine_template_fingerprint,
            machine_service_reach: Some(machine_service_reach),
            machine_synchronous_invocations: Some(machine_synchronous_invocations),
            machine_may_suspend: Some(machine_may_suspend),
            machine_may_block: Some(machine_may_block),
            machine_terminates_guarantee: Some(machine_terminates_guarantee),
            machine_crash_routes: Some(machine_crash_routes),
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
                machine_template_fingerprint: None,
                machine_service_reach: None,
                machine_synchronous_invocations: None,
                machine_may_suspend: None,
                machine_may_block: None,
                machine_terminates_guarantee: None,
                machine_crash_routes: None,
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

fn accepted_machine_may_block(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<bool, Diagnostic> {
    let plan = checked.facts.blocking.for_machine(machine).ok_or_else(|| {
        Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no exact checked blocking facts"
        ))
    })?;
    let psi_language_semantics::BlockingInterface::PublishedMayBlock(may_block) = plan.interface
    else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published blocking ceiling"
        )));
    };
    Ok(may_block)
}

fn accepted_machine_terminates_guarantee(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<bool, Diagnostic> {
    let plan = checked
        .facts
        .termination
        .for_machine(machine)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked termination facts"
            ))
        })?;
    match &plan.interface {
        psi_language_semantics::TerminationInterface::InternalDerived => Err(Diagnostic::error(
            format!("accepted machine `{machine_name}` has no published termination interface"),
        )),
        psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::NoGuarantee,
        ) => Ok(false),
        psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::Terminates { premises },
        ) if premises.is_empty() => Ok(true),
        psi_language_semantics::TerminationInterface::Published(
            psi_language_semantics::TerminationGuarantee::Terminates { .. },
        ) => Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has a progress-premised termination guarantee that cannot enter the premise-free trust row"
        ))),
    }
}

fn accepted_machine_crash_routes(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<Vec<TrustCrashRouteBucket>, Diagnostic> {
    let plan = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked crash plan"
            ))
        })?;
    if plan.crash.interface() != psi_checked_trees::CrashInterface::PublishedCeiling {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published crash ceiling"
        )));
    }
    plan.crash
        .published()
        .iter()
        .map(|bucket| {
            if bucket.alternative_guards().is_empty() {
                return Err(Diagnostic::error(format!(
                    "accepted machine `{machine_name}` has an empty published crash guard bucket"
                )));
            }
            Ok(TrustCrashRouteBucket {
                cause: match bucket.cause() {
                    psi_checked_trees::CrashCause::Trap => TrustCrashCause::Trap,
                    psi_checked_trees::CrashCause::Abort => TrustCrashCause::Abort,
                },
                alternative_guards: bucket
                    .alternative_guards()
                    .iter()
                    .map(|guard| match guard {
                        psi_checked_trees::CrashRouteGuard::Truth => TrustCrashRouteGuard::Truth,
                        psi_checked_trees::CrashRouteGuard::Predicate(predicate) => {
                            TrustCrashRouteGuard::PredicateIdentity(
                                predicate.canonical_bytes().to_vec(),
                            )
                        }
                    })
                    .collect(),
            })
        })
        .collect()
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
    use omega_artifacts::{TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard};
    use psi_checked_trees::{
        CheckedTrees, CrashCause, CrashPlan, CrashPredicateIdentity, CrashRouteBucket,
        CrashRouteGuard, MachineBlockingFact, MachineContractPlan, MachineContractPlans,
        MachineServiceReachRows, MachineSuspensionFact, MachineSynchronousInvocationFact,
        MachineTerminationFact, ServiceReachFacts,
    };
    use psi_language_semantics::{
        BlockingInterface, BlockingPlan, MachineTerminationPlan, ProgressProfileId, RankingWitness,
        ServiceReachInterface, ServiceReachRowTable, ServiceReachTable, SuspensionInterface,
        SuspensionPlan, SynchronousInvocationInterface, SynchronousInvocationPlan,
        TerminationGuarantee, TerminationInterface,
    };
    use psi_symbols::SymbolHandle;

    use super::{
        GenericAcceptedTemplateFingerprint, GenericAcceptedTemplateFingerprints,
        accepted_machine_crash_routes, accepted_machine_may_block, accepted_machine_may_suspend,
        accepted_machine_service_reach, accepted_machine_synchronous_invocations,
        accepted_machine_terminates_guarantee,
    };

    #[test]
    fn accepted_template_classification_fails_closed_on_missing_and_duplicate_symbols() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing = GenericAcceptedTemplateFingerprints { rows: Vec::new() }
            .for_machine(machine, "admitted")
            .expect_err("missing pre-lowering classification must fail closed");
        assert!(
            missing
                .message
                .contains("no pre-lowering template classification")
        );

        let duplicate = GenericAcceptedTemplateFingerprints {
            rows: vec![
                GenericAcceptedTemplateFingerprint {
                    machine,
                    fingerprint: Some(1),
                },
                GenericAcceptedTemplateFingerprint {
                    machine,
                    fingerprint: Some(1),
                },
            ],
        }
        .for_machine(machine, "admitted")
        .expect_err("duplicate pre-lowering classifications must fail closed");
        assert!(
            duplicate
                .message
                .contains("duplicate pre-lowering template classifications")
        );
    }

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

    fn checked_with_crash(machine: SymbolHandle, crash: CrashPlan) -> CheckedTrees {
        let mut checked = CheckedTrees::default();
        checked.facts.contract_plans = MachineContractPlans {
            machines: vec![MachineContractPlan {
                machine,
                closed_scalar_values: Default::default(),
                crash,
                fingerprint: 0,
            }],
            crash_capsules: Vec::new(),
        };
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

    #[test]
    fn accepted_blocking_copies_only_the_published_interface_bit() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut checked = CheckedTrees::default();
        checked.facts.blocking.machines.push(MachineBlockingFact {
            machine,
            plan: BlockingPlan {
                interface: BlockingInterface::PublishedMayBlock(true),
                checked_may_block: false,
            },
        });

        assert_eq!(
            accepted_machine_may_block(&checked, machine, "accepted"),
            Ok(true)
        );
    }

    #[test]
    fn accepted_blocking_fails_closed_on_missing_and_internal_facts() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing = accepted_machine_may_block(&CheckedTrees::default(), machine, "missing")
            .expect_err("missing facts reject")
            .to_string();
        assert!(missing.contains("has no exact checked blocking facts"));

        let mut internal = CheckedTrees::default();
        internal.facts.blocking.machines.push(MachineBlockingFact {
            machine,
            plan: BlockingPlan {
                interface: BlockingInterface::InternalInferred,
                checked_may_block: true,
            },
        });
        let internal = accepted_machine_may_block(&internal, machine, "internal")
            .expect_err("private inference rejects")
            .to_string();
        assert!(internal.contains("has no published blocking ceiling"));
    }

    #[test]
    fn accepted_termination_copies_only_the_premise_free_published_interface() {
        let machine = SymbolHandle::from_arena_index(1);
        let mut checked = CheckedTrees::default();
        checked
            .facts
            .termination
            .machines
            .push(MachineTerminationFact {
                machine,
                plan: MachineTerminationPlan {
                    interface: TerminationInterface::Published(TerminationGuarantee::Terminates {
                        premises: Vec::new(),
                    }),
                    checked_summary: TerminationGuarantee::NoGuarantee,
                    implementation_witness: Some(RankingWitness {
                        view_path: "Private::Witness".to_owned(),
                        ..Default::default()
                    }),
                },
            });

        assert_eq!(
            accepted_machine_terminates_guarantee(&checked, machine, "accepted"),
            Ok(true)
        );
    }

    #[test]
    fn accepted_termination_fails_closed_on_missing_internal_and_premised_facts() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing =
            accepted_machine_terminates_guarantee(&CheckedTrees::default(), machine, "missing")
                .expect_err("missing facts reject")
                .to_string();
        assert!(missing.contains("has no exact checked termination facts"));

        let mut internal = CheckedTrees::default();
        internal
            .facts
            .termination
            .machines
            .push(MachineTerminationFact {
                machine,
                plan: MachineTerminationPlan {
                    interface: TerminationInterface::InternalDerived,
                    checked_summary: TerminationGuarantee::Terminates {
                        premises: Vec::new(),
                    },
                    implementation_witness: None,
                },
            });
        let internal = accepted_machine_terminates_guarantee(&internal, machine, "internal")
            .expect_err("private derivation rejects")
            .to_string();
        assert!(internal.contains("has no published termination interface"));

        let mut premised = CheckedTrees::default();
        premised
            .facts
            .termination
            .machines
            .push(MachineTerminationFact {
                machine,
                plan: MachineTerminationPlan {
                    interface: TerminationInterface::Published(TerminationGuarantee::Terminates {
                        premises: vec![ProgressProfileId(1)],
                    }),
                    checked_summary: TerminationGuarantee::NoGuarantee,
                    implementation_witness: None,
                },
            });
        let premised = accepted_machine_terminates_guarantee(&premised, machine, "premised")
            .expect_err("progress-premised guarantee rejects")
            .to_string();
        assert!(premised.contains("cannot enter the premise-free trust row"));
    }

    #[test]
    fn accepted_crash_routes_copy_exact_published_bucket_and_guard_identity() {
        let machine = SymbolHandle::from_arena_index(1);
        let trap = CrashRouteBucket::new(
            CrashCause::Trap,
            vec![CrashRouteGuard::Predicate(
                CrashPredicateIdentity::from_canonical_bytes(vec![1, 2]),
            )],
        )
        .expect("nonempty guarded bucket");
        let abort = CrashRouteBucket::unconditional(CrashCause::Abort);
        let checked = checked_with_crash(machine, CrashPlan::published_ceiling(vec![abort, trap]));

        assert_eq!(
            accepted_machine_crash_routes(&checked, machine, "accepted"),
            Ok(vec![
                TrustCrashRouteBucket {
                    cause: TrustCrashCause::Trap,
                    alternative_guards: vec![TrustCrashRouteGuard::PredicateIdentity(vec![1, 2])],
                },
                TrustCrashRouteBucket {
                    cause: TrustCrashCause::Abort,
                    alternative_guards: vec![TrustCrashRouteGuard::Truth],
                },
            ])
        );
    }

    #[test]
    fn accepted_crash_routes_fail_closed_on_missing_and_internal_plans() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing = accepted_machine_crash_routes(&CheckedTrees::default(), machine, "missing")
            .expect_err("missing plan rejects")
            .to_string();
        assert!(missing.contains("has no exact checked crash plan"));

        let internal = checked_with_crash(machine, CrashPlan::default());
        let internal = accepted_machine_crash_routes(&internal, machine, "internal")
            .expect_err("private inference rejects")
            .to_string();
        assert!(internal.contains("has no published crash ceiling"));

        assert_eq!(
            CrashRouteBucket::new(CrashCause::Trap, Vec::new()),
            None,
            "the checked owner seals the empty-guard state before trust projection"
        );
    }
}
