//! GR5/GR6 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment, plus exact provider-requirement and routed
//! qualification rows copied from normalized provider plans. Accepted facts,
//! provider plans, and their requirement blast radius retain root-grant or
//! dev-active provenance; the latter carries a standing warning. Domains are
//! semantic declarations, not grantable trust-report subjects.

use omega_artifacts::{
    ArtifactWriter, TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard,
    TrustGenericAcceptedInstanceRow, TrustProgressPremiseRow, TrustProgressPremiseSubject,
    TrustProviderRealization, TrustProviderRequirementRow, TrustQualificationRow, TrustReport,
    TrustReportRow,
};
use psi_diagnostics::Diagnostic;

pub fn write_trust_report(
    build_dir: &std::path::Path,
    checked: &psi_checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    accepted_template_classifications: &crate::AcceptedTemplateClassifications,
    emit_auxiliary_artifacts: bool,
) -> Result<(), Vec<Diagnostic>> {
    let typed = &checked.typed;
    let mut report = TrustReport::default();
    report.selected_provider_closure_fingerprint = selected_provider_plans.normalized_identity();
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let mut non_provider_grants = Vec::new();
    for grant in root_grants {
        if provider_grants
            .iter()
            .any(|provider_grant| provider_grant.selector == *grant)
        {
            continue;
        }
        non_provider_grants.push((
            grant.as_str(),
            crate::lockfile::resolve_non_provider_trust_grant(typed, grant)
                .map_err(|diagnostic| vec![diagnostic])?,
        ));
    }
    // PRV3: derived provider plans -- one row each, dev-active with the
    // standing warning until the final build grants the plan by exact name or
    // exact selected slot, fingerprint shown so drift is visible at a glance.
    for plan in provider_plans {
        let selected = selected_provider_plans
            .plan_by_identity(plan.identity_fingerprint())
            .is_some();
        let grant_selectors = selected
            .then(|| {
                provider_grants
                    .iter()
                    .filter(|grant| grant.selected_plan_identity == plan.identity_fingerprint())
                    .map(|grant| grant.selector.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
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
                "provider plan: {} [{:016x}] provider type: {} target: {} provider origin package: {} provider package key: {} coverage {covered}/{} selected: {}",
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
                if plan.origin_package.is_empty() {
                    "<none>"
                } else {
                    plan.origin_package.as_str()
                },
                package_key_text(plan.origin_package_identity),
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
                    provider_type_package_identity: plan.provider_type_package_identity,
                    target: plan.target.clone(),
                    provider_origin_package_identity: plan.origin_package_identity,
                    provider_origin_package: plan.origin_package.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    service_schema_package_identity: plan.schema.trait_package_identity,
                    calling_plan_fingerprint: method.calling_plan_fingerprint,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_owner_package_identity: method
                        .requirement_owner_package_identity,
                    requirement_identity: row.requirement_identity.clone(),
                    method: row.method.clone(),
                    parameter_type_identities: method.parameter_type_identities.clone(),
                    result_type_identity: method.result_type_identity.clone(),
                    service_reach: method.service_reach.clone(),
                    synchronous_invocations: method.synchronous_invocations.clone(),
                    may_suspend: method.may_suspend,
                    may_block: method.may_block,
                    terminates_guarantee: method.terminates_guarantee,
                    termination_premises: method
                        .termination_premises
                        .iter()
                        .map(|premise| TrustProgressPremiseRow {
                            profile: premise.profile.clone(),
                            subject: match premise.subject {
                                omega_effects::provider_plan::ServiceProgressSubject::ProviderReceiver => {
                                    TrustProgressPremiseSubject::ProviderReceiver
                                }
                                omega_effects::provider_plan::ServiceProgressSubject::Parameter(index) => {
                                    TrustProgressPremiseSubject::Parameter(index)
                                }
                            },
                            subject_projections: premise.subject_projections.clone(),
                        })
                        .collect(),
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
                    provider_type_package_identity: plan.provider_type_package_identity,
                    target: plan.target.clone(),
                    provider_origin_package_identity: plan.origin_package_identity,
                    provider_origin_package: plan.origin_package.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    service_schema_package_identity: plan.schema.trait_package_identity,
                    calling_plan_fingerprint: method.calling_plan_fingerprint,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_owner_package_identity: method.requirement_owner_package_identity,
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
                    provider_type_package_identity: plan.provider_type_package_identity,
                    target: plan.target.clone(),
                    provider_origin_package_identity: plan.origin_package_identity,
                    provider_origin_package: plan.origin_package.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    service_schema_package_identity: plan.schema.trait_package_identity,
                    calling_plan_fingerprint: method.calling_plan_fingerprint,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_owner_package_identity: method.requirement_owner_package_identity,
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
    // ACCEPTED machines (bodyless boundary axioms, GR6d): one row each --
    // own-package dev-active with the standing warning, or root-granted
    // when build.omg names the machine.
    for machine in typed.machines() {
        if machine.supply_mode != psi_language_semantics::MachineSupplyMode::Accepted {
            continue;
        }
        // A generic accepted template spends and reports one universal
        // commitment. Later specializations may clone its machine under fresh
        // symbols; those clones belong exclusively to the exact instance
        // section below and must not mint duplicate commitment rows.
        if typed.machine_specializations.iter().any(|specialization| {
            specialization.accepted_template_commitment.is_some()
                && specialization.instance == machine.symbol
                && specialization.instance != specialization.template
        }) {
            continue;
        }
        let granted = non_provider_grants.iter().any(|(_, subject)| {
            *subject == crate::lockfile::NonProviderTrustGrant::AcceptedMachine(machine.symbol)
        });
        let contract = exact_machine_contract_plan(
            checked,
            machine.symbol,
            &format!("accepted machine `{}`", machine.name.as_str()),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let machine_contract_fingerprint = contract.fingerprint;
        let machine_template_fingerprint = accepted_template_classifications
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
        let machine_crash_routes = accepted_machine_crash_routes(contract, machine.name.as_str())
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
    for specialization in &typed.machine_specializations {
        let Some(template_commitment) = specialization.accepted_template_commitment.as_ref() else {
            continue;
        };
        let instance_contract_fingerprint = accepted_instance_contract_fingerprint(
            checked,
            specialization.instance,
            template_commitment,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        report
            .generic_accepted_instances
            .push(TrustGenericAcceptedInstanceRow {
                template_commitment: template_commitment.clone(),
                template_fingerprint: specialization.template_contract_fingerprint,
                instance_fingerprint: specialization.fingerprint,
                instance_contract_fingerprint,
                type_argument_identities: specialization.type_argument_identities.clone(),
                const_argument_identities: specialization.const_argument_identities.clone(),
                machine_argument_contract_fingerprints: specialization
                    .machine_argument_contract_fingerprints
                    .clone(),
                conformance_argument_fingerprints: specialization
                    .conformance_argument_fingerprints
                    .clone(),
            });
    }

    if emit_auxiliary_artifacts {
        let writer = ArtifactWriter::new(build_dir).map_err(|diagnostic| vec![diagnostic])?;
        writer
            .write_trust_report(&report)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    Ok(())
}

fn package_key_text(identity: Option<psi_core::PackageKeyIdentity>) -> String {
    let Some(identity) = identity else {
        return "<unbound>".to_owned();
    };
    identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn accepted_instance_contract_fingerprint(
    checked: &psi_checked_trees::CheckedTrees,
    instance: psi_symbols::SymbolHandle,
    template_commitment: &str,
) -> Result<u64, Diagnostic> {
    exact_machine_contract_plan(
        checked,
        instance,
        &format!("accepted generic instance of `{template_commitment}`"),
    )
    .map(|plan| plan.fingerprint)
}

fn exact_machine_contract_plan<'checked>(
    checked: &'checked psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    subject: &str,
) -> Result<&'checked psi_checked_trees::MachineContractPlan, Diagnostic> {
    let mut matches = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .filter(|plan| plan.machine == machine);
    let plan = matches.next().ok_or_else(|| {
        Diagnostic::error(format!("{subject} has no exact checked contract plan"))
    })?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "{subject} has duplicate exact checked contract plans"
        )));
    }
    Ok(plan)
}

fn accepted_machine_service_reach(
    checked: &psi_checked_trees::CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    machine_name: &str,
) -> Result<Vec<String>, Diagnostic> {
    let mut matches = checked
        .facts
        .service_reaches
        .machines()
        .iter()
        .filter(|fact| fact.machine == machine);
    let machine_reach = matches.next().ok_or_else(|| {
        Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no exact checked service-reach facts"
        ))
    })?;
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has duplicate exact checked service-reach facts"
        )));
    }
    let psi_language_semantics::ServiceReachInterface::PublishedCeiling(reach_row) =
        machine_reach.interface
    else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published service-reach ceiling"
        )));
    };
    let services = checked.facts.service_reaches.rows.services(reach_row);
    if services.is_empty() && reach_row != psi_language_semantics::ServiceReachRowTable::EMPTY_ROW {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` references an unknown service-reach row identity"
        )));
    }
    services
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
    let mut matches = checked
        .facts
        .synchronous_invocations
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = &matches
        .next()
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked synchronous-invocation facts"
            ))
        })?
        .plan;
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has duplicate exact checked synchronous-invocation facts"
        )));
    }
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
    let mut matches = checked
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = matches
        .next()
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked suspension facts"
            ))
        })?
        .plan;
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has duplicate exact checked suspension facts"
        )));
    }
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
    let mut matches = checked
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = matches
        .next()
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked blocking facts"
            ))
        })?
        .plan;
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has duplicate exact checked blocking facts"
        )));
    }
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
    let mut matches = checked
        .facts
        .termination
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let plan = &matches
        .next()
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "accepted machine `{machine_name}` has no exact checked termination facts"
            ))
        })?
        .plan;
    if matches.next().is_some() {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has duplicate exact checked termination facts"
        )));
    }
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
    plan: &psi_checked_trees::MachineContractPlan,
    machine_name: &str,
) -> Result<Vec<TrustCrashRouteBucket>, Diagnostic> {
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

    let realization = match binding {
        ProviderBinding::Import { locator } => TrustProviderRealization::Import {
            locator: locator.clone(),
        },
        ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
            TrustProviderRealization::StringBackedImportBootstrap {
                library: library.clone(),
                symbol: symbol.clone(),
            }
        }
        ProviderBinding::Syscall { number } => {
            TrustProviderRealization::Syscall { number: *number }
        }
        ProviderBinding::CompilerIntrinsic { machine, .. } => {
            TrustProviderRealization::CompilerIntrinsic {
                machine: machine.clone(),
            }
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
        ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity,
        } => TrustProviderRealization::CheckedAdapter {
            machine_identity: machine_identity.clone(),
            machine_package_identity: *machine_package_identity,
        },
    };
    realization
}

#[cfg(test)]
mod tests {
    use super::{
        accepted_instance_contract_fingerprint, accepted_machine_crash_routes,
        accepted_machine_may_block, accepted_machine_may_suspend, accepted_machine_service_reach,
        accepted_machine_synchronous_invocations, accepted_machine_terminates_guarantee,
        exact_machine_contract_plan, trust_provider_realization,
    };
    use omega_artifacts::{
        TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard, TrustProviderRealization,
    };
    use psi_checked_trees::{
        CheckedTrees, CrashCause, CrashPlan, CrashPredicateIdentity, CrashRouteBucket,
        CrashRouteGuard, MachineBlockingFact, MachineContractPlan, MachineContractPlans,
        MachineServiceReachRows, MachineSuspensionFact, MachineSynchronousInvocationFact,
        MachineTerminationFact, ServiceReachFacts,
    };
    use psi_language_semantics::{
        BlockingInterface, BlockingPlan, MachineTerminationPlan, ProgressPremise, ProgressSubject,
        RankingWitness, SemanticDomainId, ServiceReachInterface, ServiceReachRowId,
        ServiceReachRowTable, ServiceReachTable, SuspensionInterface, SuspensionPlan,
        SynchronousInvocationInterface, SynchronousInvocationPlan, TerminationGuarantee,
        TerminationInterface,
    };
    use psi_symbols::SymbolHandle;

    #[test]
    fn trust_realization_retains_normalized_locator_and_keeps_bootstrap_distinct() {
        let locator = omega_effects::normalize_foreign_locator(
            omega_effects::ForeignLocatorCandidate::ElfVersioned {
                object: b"libopaque.so".to_vec(),
                symbol: b"invoke_raw".to_vec(),
                version: b"OPAQUE_2.0".to_vec(),
            },
            omega_target::TargetProfile::LinuxX64,
        )
        .expect("valid normalized Linux import");
        let normalized =
            trust_provider_realization(&omega_effects::provider_plan::ProviderBinding::Import {
                locator: locator.clone(),
            });
        assert_eq!(
            normalized,
            TrustProviderRealization::Import {
                locator: locator.clone(),
            }
        );
        assert_eq!(
            normalized.normalized_foreign_locator_identity(),
            Some(locator.normalized_identity()),
        );

        let bootstrap = trust_provider_realization(
            &omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap {
                library: "libopaque.so".to_owned(),
                symbol: "invoke_raw".to_owned(),
            },
        );
        assert_eq!(
            bootstrap,
            TrustProviderRealization::StringBackedImportBootstrap {
                library: "libopaque.so".to_owned(),
                symbol: "invoke_raw".to_owned(),
            }
        );
        assert_eq!(bootstrap.normalized_foreign_locator_identity(), None);
    }

    #[test]
    fn accepted_instance_contract_identity_copies_exact_plan_and_fails_closed_when_missing() {
        let machine = SymbolHandle::from_arena_index(1);
        let missing =
            accepted_instance_contract_fingerprint(&CheckedTrees::default(), machine, "admitted")
                .expect_err("missing checked instance plan must fail closed");
        assert!(
            missing
                .message
                .contains("has no exact checked contract plan")
        );

        let mut checked = CheckedTrees::default();
        checked
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine,
                closed_scalar_values: Default::default(),
                crash: CrashPlan::default(),
                fingerprint: 0x1234_5678_9abc_def0,
            });
        assert_eq!(
            accepted_instance_contract_fingerprint(&checked, machine, "admitted"),
            Ok(0x1234_5678_9abc_def0)
        );

        let duplicate = checked.facts.contract_plans.machines[0].clone();
        checked.facts.contract_plans.machines.push(duplicate);
        let duplicate = accepted_instance_contract_fingerprint(&checked, machine, "admitted")
            .expect_err("duplicate checked instance plans must fail closed");
        assert!(
            duplicate
                .message
                .contains("duplicate exact checked contract plans")
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
            realized_envelopes: Vec::new(),
        };
        checked
    }

    #[derive(Clone, Copy)]
    enum AcceptedTrustAxis {
        Contract,
        ServiceReach,
        SynchronousInvocation,
        Suspension,
        Blocking,
        Termination,
    }

    impl AcceptedTrustAxis {
        const ALL: [Self; 6] = [
            Self::Contract,
            Self::ServiceReach,
            Self::SynchronousInvocation,
            Self::Suspension,
            Self::Blocking,
            Self::Termination,
        ];

        fn label(self) -> &'static str {
            match self {
                Self::Contract => "contract",
                Self::ServiceReach => "service reach",
                Self::SynchronousInvocation => "synchronous invocation",
                Self::Suspension => "suspension",
                Self::Blocking => "blocking",
                Self::Termination => "termination",
            }
        }
    }

    fn accepted_exact_rows_fixture(machine: SymbolHandle) -> CheckedTrees {
        let mut checked = CheckedTrees::default();
        checked
            .facts
            .contract_plans
            .machines
            .push(MachineContractPlan {
                machine,
                closed_scalar_values: Default::default(),
                crash: CrashPlan::published_ceiling(Vec::new()),
                fingerprint: 0x1234,
            });
        let empty_reach = checked.facts.service_reaches.rows.intern(Vec::new());
        checked.facts.service_reaches.machines.append_to_span(
            &mut checked.facts.service_reaches.root_machines,
            MachineServiceReachRows {
                machine,
                interface: ServiceReachInterface::PublishedCeiling(empty_reach),
                ..Default::default()
            },
        );
        checked
            .facts
            .synchronous_invocations
            .machines
            .push(MachineSynchronousInvocationFact {
                machine,
                published_targets: Vec::new(),
                checked_inferred_targets: Vec::new(),
                plan: SynchronousInvocationPlan {
                    interface: SynchronousInvocationInterface::PublishedCeiling,
                    published: Vec::new(),
                    checked_inferred: vec!["service:Private".to_owned()],
                },
            });
        checked
            .facts
            .suspensions
            .machines
            .push(MachineSuspensionFact {
                machine,
                plan: SuspensionPlan {
                    interface: SuspensionInterface::PublishedMaySuspend(false),
                    checked_may_suspend: true,
                },
            });
        checked.facts.blocking.machines.push(MachineBlockingFact {
            machine,
            plan: BlockingPlan {
                interface: BlockingInterface::PublishedMayBlock(false),
                checked_may_block: true,
            },
        });
        checked
            .facts
            .termination
            .machines
            .push(MachineTerminationFact {
                machine,
                plan: MachineTerminationPlan {
                    interface: TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                    checked_summary: TerminationGuarantee::Terminates {
                        premises: Vec::new(),
                    },
                    implementation_witness: Some(RankingWitness {
                        view_path: "Private::Witness".to_owned(),
                        ..Default::default()
                    }),
                },
            });
        checked
    }

    fn validate_accepted_axis(
        checked: &CheckedTrees,
        machine: SymbolHandle,
        axis: AcceptedTrustAxis,
    ) -> Result<(), String> {
        let result = match axis {
            AcceptedTrustAxis::Contract => {
                exact_machine_contract_plan(checked, machine, "accepted machine `accepted`")
                    .map(|_| ())
            }
            AcceptedTrustAxis::ServiceReach => {
                accepted_machine_service_reach(checked, machine, "accepted").map(|_| ())
            }
            AcceptedTrustAxis::SynchronousInvocation => {
                accepted_machine_synchronous_invocations(checked, machine, "accepted").map(|_| ())
            }
            AcceptedTrustAxis::Suspension => {
                accepted_machine_may_suspend(checked, machine, "accepted").map(|_| ())
            }
            AcceptedTrustAxis::Blocking => {
                accepted_machine_may_block(checked, machine, "accepted").map(|_| ())
            }
            AcceptedTrustAxis::Termination => {
                accepted_machine_terminates_guarantee(checked, machine, "accepted").map(|_| ())
            }
        };
        result.map_err(|diagnostic| diagnostic.message)
    }

    fn remove_accepted_axis(
        checked: &mut CheckedTrees,
        machine: SymbolHandle,
        axis: AcceptedTrustAxis,
    ) {
        match axis {
            AcceptedTrustAxis::Contract => checked
                .facts
                .contract_plans
                .machines
                .retain(|row| row.machine != machine),
            AcceptedTrustAxis::ServiceReach => {
                checked
                    .facts
                    .service_reaches
                    .machines
                    .for_each_mut(|_, row| {
                        if row.machine == machine {
                            row.machine = SymbolHandle::invalid();
                        }
                    });
            }
            AcceptedTrustAxis::SynchronousInvocation => checked
                .facts
                .synchronous_invocations
                .machines
                .retain(|row| row.machine != machine),
            AcceptedTrustAxis::Suspension => checked
                .facts
                .suspensions
                .machines
                .retain(|row| row.machine != machine),
            AcceptedTrustAxis::Blocking => checked
                .facts
                .blocking
                .machines
                .retain(|row| row.machine != machine),
            AcceptedTrustAxis::Termination => checked
                .facts
                .termination
                .machines
                .retain(|row| row.machine != machine),
        }
    }

    fn append_accepted_axis_copy(
        checked: &mut CheckedTrees,
        source: SymbolHandle,
        owner: SymbolHandle,
        axis: AcceptedTrustAxis,
    ) {
        match axis {
            AcceptedTrustAxis::Contract => {
                let mut row = checked
                    .facts
                    .contract_plans
                    .machines
                    .iter()
                    .find(|row| row.machine == source)
                    .expect("source contract row")
                    .clone();
                row.machine = owner;
                checked.facts.contract_plans.machines.push(row);
            }
            AcceptedTrustAxis::ServiceReach => {
                let mut row = checked
                    .facts
                    .service_reaches
                    .machines()
                    .iter()
                    .find(|row| row.machine == source)
                    .expect("source service-reach row")
                    .clone();
                row.machine = owner;
                checked
                    .facts
                    .service_reaches
                    .machines
                    .append_to_span(&mut checked.facts.service_reaches.root_machines, row);
            }
            AcceptedTrustAxis::SynchronousInvocation => {
                let mut row = checked
                    .facts
                    .synchronous_invocations
                    .machines
                    .iter()
                    .find(|row| row.machine == source)
                    .expect("source synchronous-invocation row")
                    .clone();
                row.machine = owner;
                checked.facts.synchronous_invocations.machines.push(row);
            }
            AcceptedTrustAxis::Suspension => {
                let mut row = *checked
                    .facts
                    .suspensions
                    .machines
                    .iter()
                    .find(|row| row.machine == source)
                    .expect("source suspension row");
                row.machine = owner;
                checked.facts.suspensions.machines.push(row);
            }
            AcceptedTrustAxis::Blocking => {
                let mut row = *checked
                    .facts
                    .blocking
                    .machines
                    .iter()
                    .find(|row| row.machine == source)
                    .expect("source blocking row");
                row.machine = owner;
                checked.facts.blocking.machines.push(row);
            }
            AcceptedTrustAxis::Termination => {
                let mut row = checked
                    .facts
                    .termination
                    .machines
                    .iter()
                    .find(|row| row.machine == source)
                    .expect("source termination row")
                    .clone();
                row.machine = owner;
                checked.facts.termination.machines.push(row);
            }
        }
    }

    #[test]
    fn accepted_trust_axes_require_one_exact_row_without_global_poisoning() {
        let machine = SymbolHandle::from_arena_index(1);
        let unrelated = SymbolHandle::from_arena_index(2);
        for axis in AcceptedTrustAxis::ALL {
            let mut missing = accepted_exact_rows_fixture(machine);
            remove_accepted_axis(&mut missing, machine, axis);
            let error = validate_accepted_axis(&missing, machine, axis)
                .expect_err("missing exact row must fail closed");
            assert!(
                error.contains("no exact checked"),
                "{} missing diagnostic: {error}",
                axis.label()
            );

            let mut duplicate = accepted_exact_rows_fixture(machine);
            append_accepted_axis_copy(&mut duplicate, machine, machine, axis);
            let error = validate_accepted_axis(&duplicate, machine, axis)
                .expect_err("duplicate exact row must fail closed");
            assert!(
                error.contains("duplicate exact checked"),
                "{} duplicate diagnostic: {error}",
                axis.label()
            );

            let mut unrelated_duplicates = accepted_exact_rows_fixture(machine);
            append_accepted_axis_copy(&mut unrelated_duplicates, machine, unrelated, axis);
            append_accepted_axis_copy(&mut unrelated_duplicates, machine, unrelated, axis);
            validate_accepted_axis(&unrelated_duplicates, machine, axis).unwrap_or_else(|error| {
                panic!("{} unrelated rows must be ignored: {error}", axis.label())
            });
        }
    }

    #[test]
    fn accepted_trust_axes_preserve_explicit_public_negatives() {
        let machine = SymbolHandle::from_arena_index(1);
        let checked = accepted_exact_rows_fixture(machine);
        assert_eq!(
            accepted_machine_service_reach(&checked, machine, "accepted"),
            Ok(Vec::new())
        );
        assert_eq!(
            accepted_machine_synchronous_invocations(&checked, machine, "accepted"),
            Ok(Vec::new())
        );
        assert_eq!(
            accepted_machine_may_suspend(&checked, machine, "accepted"),
            Ok(false)
        );
        assert_eq!(
            accepted_machine_may_block(&checked, machine, "accepted"),
            Ok(false)
        );
        assert_eq!(
            accepted_machine_terminates_guarantee(&checked, machine, "accepted"),
            Ok(false)
        );
        let contract =
            exact_machine_contract_plan(&checked, machine, "accepted machine `accepted`")
                .expect("one exact contract");
        assert_eq!(
            accepted_machine_crash_routes(contract, "accepted"),
            Ok(Vec::new())
        );
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

        let invalid_row = checked_with_reach(
            machine,
            ServiceReachInterface::PublishedCeiling(ServiceReachRowId(99)),
            ServiceReachTable::default(),
            ServiceReachRowTable::default(),
        );
        let invalid_row = accepted_machine_service_reach(&invalid_row, machine, "invalid-row")
            .expect_err("unknown row identity rejects")
            .to_string();
        assert!(invalid_row.contains("references an unknown service-reach row identity"));
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
                published_targets: Vec::new(),
                checked_inferred_targets: Vec::new(),
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
                published_targets: Vec::new(),
                checked_inferred_targets: Vec::new(),
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
                        premises: vec![ProgressPremise {
                            profile: SemanticDomainId(1),
                            subject: ProgressSubject {
                                root: machine,
                                projections: Vec::new(),
                            },
                        }],
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
        let contract =
            exact_machine_contract_plan(&checked, machine, "accepted machine `accepted`")
                .expect("one exact contract");

        assert_eq!(
            accepted_machine_crash_routes(contract, "accepted"),
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
        let missing = exact_machine_contract_plan(
            &CheckedTrees::default(),
            machine,
            "accepted machine `missing`",
        )
        .expect_err("missing plan rejects")
        .to_string();
        assert!(missing.contains("has no exact checked contract plan"));

        let internal = checked_with_crash(machine, CrashPlan::default());
        let contract =
            exact_machine_contract_plan(&internal, machine, "accepted machine `internal`")
                .expect("one exact contract");
        let internal = accepted_machine_crash_routes(contract, "internal")
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
