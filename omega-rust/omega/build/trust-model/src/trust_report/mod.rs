//! GR5/GR6 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment, plus exact provider-requirement and routed
//! qualification rows copied from normalized provider plans. Accepted facts,
//! provider plans, and their requirement blast radius retain root-grant or
//! dev-active provenance; the latter carries a standing warning. Domains are
//! semantic declarations, not grantable trust-report subjects.

use artifacts::{
    TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard, TrustGenericAcceptedInstanceRow,
    TrustProgressPremiseRow, TrustProgressPremiseSubject, TrustProviderRealization,
    TrustProviderRequirementRow, TrustQualificationRow, TrustReport, TrustReportRow,
};
use diagnostics::Diagnostic;

pub fn reconstruct_trust_report(
    checked: &checked_trees::CheckedTrees,
    root_grants: &[String],
    provider_plans: &[effects::provider_plan::ProviderPlan],
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    accepted_template_classifications: &crate::AcceptedTemplateClassifications,
) -> Result<TrustReport, Vec<Diagnostic>> {
    let typed = &checked.typed;
    let mut report = TrustReport {
        selected_provider_closure_report_fingerprint: selected_provider_plans
            .compatibility_report_identity(),
        selected_provider_closure_digest: selected_provider_plans.identity_digest(),
        ..Default::default()
    };
    for selected_plan in selected_provider_plans.plans() {
        let exact_candidate_matches = provider_plans
            .iter()
            .filter(|candidate| {
                *candidate == selected_plan
                    && candidate.identity_digest() == selected_plan.identity_digest()
            })
            .count();
        if exact_candidate_matches != 1 {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` replays against {exact_candidate_matches} exact candidate plans",
                selected_plan.name,
            ))]);
        }
    }
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    for grant in &provider_grants {
        let exact_selected_matches = selected_provider_plans
            .plans()
            .iter()
            .filter(|plan| grant.replays_selected_plan(plan))
            .count();
        if exact_selected_matches != 1 {
            return Err(vec![Diagnostic::error(format!(
                "provider grant `{}` replays against {exact_selected_matches} exact selected provider plans",
                grant.selector,
            ))]);
        }
    }
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
            crate::resolve_non_provider_trust_grant(typed, grant)
                .map_err(|diagnostic| vec![diagnostic])?,
        ));
    }
    // PRV3: derived provider plans -- one row each, dev-active with the
    // standing warning until the final build grants the plan by exact name or
    // exact selected slot, report fingerprint shown so drift is visible at a
    // glance while exact structure and the strong digest classify authority.
    for plan in provider_plans {
        let selected = selected_provider_plans.plans().iter().any(|selected| {
            selected == plan && selected.identity_digest() == plan.identity_digest()
        });
        // Toolchain-settled plans are toolchain-owned realizations of
        // canonical boundary slots; no package grant applies or is needed,
        // so they carry their own provenance rather than the ungranted
        // dev-active warning.
        let toolchain_settled = selected && selected_provider_plans.is_toolchain_settled(plan);
        let grant_selectors = if selected {
            {
                provider_grants
                    .iter()
                    .filter(|grant| grant.replays_selected_plan(plan))
                    .map(|grant| grant.selector.clone())
                    .collect::<Vec<_>>()
            }
        } else {
            Default::default()
        };
        let granted = !grant_selectors.is_empty();
        let provenance = if toolchain_settled {
            "toolchain-settled"
        } else if granted {
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
                plan.report_fingerprint(),
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
            machine_contract_report_fingerprint: None,
            machine_contract_commitment: None,
            machine_template_report_fingerprint: None,
            machine_service_reach: None,
            machine_synchronous_invocations: None,
            machine_may_suspend: None,
            machine_may_block: None,
            machine_terminates_guarantee: None,
            machine_crash_routes: None,
            standing_warning: !granted && !toolchain_settled,
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
                    provider_plan_report_fingerprint: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                    provider_type: plan.provider_type.clone(),
                    provider_type_package_identity: plan.provider_type_package_identity,
                    target: plan.target.clone(),
                    provider_origin_package_identity: plan.origin_package_identity,
                    provider_origin_package: plan.origin_package.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    service_schema_package_identity: plan.schema.trait_package_identity,
                    calling_plan_report_fingerprint: method.calling_plan_report_fingerprint,
                    calling_plan_commitment: method.calling_plan_commitment,
                    selected,
                    requirement_owner: method.requirement_owner.clone(),
                    requirement_owner_package_identity: method.requirement_owner_package_identity,
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
                        .map(|premise| {
                            TrustProgressPremiseRow {
                        profile: premise.profile.clone(),
                        subject: match premise.subject {
                            effects::provider_plan::ServiceProgressSubject::ProviderReceiver => {
                                TrustProgressPremiseSubject::ProviderReceiver
                            }
                            effects::provider_plan::ServiceProgressSubject::Parameter(index) => {
                                TrustProgressPremiseSubject::Parameter(index)
                            }
                        },
                        subject_projections: premise.subject_projections.clone(),
                    }
                        })
                        .collect(),
                    realization: trust_provider_realization(&row.binding),
                    provenance: provenance.to_owned(),
                    grant_selectors: grant_selectors.clone(),
                    standing_warning: !granted && !toolchain_settled,
                });
        }
        // Preserve schema declaration order while excluding every unbound
        // requirement from a partial candidate's qualification blast radius.
        bound_methods.sort_unstable_by_key(|(method_index, _)| *method_index);
        for (_, method) in bound_methods {
            for claim in &method.entry_claims {
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_report_fingerprint: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                    provider_type: plan.provider_type.clone(),
                    provider_type_package_identity: plan.provider_type_package_identity,
                    target: plan.target.clone(),
                    provider_origin_package_identity: plan.origin_package_identity,
                    provider_origin_package: plan.origin_package.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    service_schema_package_identity: plan.schema.trait_package_identity,
                    calling_plan_report_fingerprint: method.calling_plan_report_fingerprint,
                    calling_plan_commitment: method.calling_plan_commitment,
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
                    standing_warning: !granted && !toolchain_settled,
                });
            }
            for claim in &method.result_claims {
                // ServiceResultClaim contains only bodyless routed results;
                // predicate-bearing establishment is deliberately absent from
                // this generic provider-result carrier.
                report.qualifications.push(TrustQualificationRow {
                    provider_plan: plan.name.clone(),
                    provider_plan_report_fingerprint: plan.report_fingerprint(),
                    provider_plan_digest: plan.identity_digest(),
                    provider_type: plan.provider_type.clone(),
                    provider_type_package_identity: plan.provider_type_package_identity,
                    target: plan.target.clone(),
                    provider_origin_package_identity: plan.origin_package_identity,
                    provider_origin_package: plan.origin_package.clone(),
                    service_schema: plan.schema.trait_name.clone(),
                    service_schema_package_identity: plan.schema.trait_package_identity,
                    calling_plan_report_fingerprint: method.calling_plan_report_fingerprint,
                    calling_plan_commitment: method.calling_plan_commitment,
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
                    standing_warning: !granted && !toolchain_settled,
                });
            }
        }
    }
    // ACCEPTED machines (bodyless boundary axioms, GR6d): one row each --
    // own-package dev-active with the standing warning, or root-granted
    // when build.omg names the machine.
    for machine in typed.machines() {
        if machine.supply_mode != language_semantics::MachineSupplyMode::AdmissionClaim {
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
            *subject == crate::NonProviderTrustGrant::AcceptedMachine(machine.symbol)
        });
        let contract = exact_machine_contract_plan(
            checked,
            machine.symbol,
            &format!("accepted machine `{}`", machine.name.as_str()),
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let machine_contract_report_fingerprint = contract.report_fingerprint;
        let machine_template_report_fingerprint = accepted_template_classifications
            .for_machine(machine.symbol, machine.name.as_str())
            .map_err(|diagnostic| vec![diagnostic])?
            .map(|identity| identity.report_fingerprint());
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
            machine_contract_report_fingerprint: Some(machine_contract_report_fingerprint),
            machine_contract_commitment: Some(contract.commitment),
            machine_template_report_fingerprint,
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
        let instance_contract =
            accepted_instance_contract_plan(checked, specialization.instance, template_commitment)
                .map_err(|diagnostic| vec![diagnostic])?;
        let machine_argument_contract_commitments = specialization
            .machine_arguments
            .iter()
            .map(|state| {
                let owner = typed.machines().iter().find(|machine| {
                    typed
                        .machine_states(machine)
                        .iter()
                        .any(|candidate| candidate.symbol == *state)
                });
                let owner = owner.ok_or_else(|| {
                    Diagnostic::error(format!(
                        "accepted generic instance of `{template_commitment}` references a static machine state with no exact owning machine"
                    ))
                })?;
                exact_machine_contract_plan(
                    checked,
                    owner.symbol,
                    &format!("static machine argument `{}`", owner.name.as_str()),
                )
                .map(|plan| plan.commitment)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|diagnostic| vec![diagnostic])?;
        report
            .generic_accepted_instances
            .push(TrustGenericAcceptedInstanceRow {
                template_commitment: template_commitment.clone(),
                template_report_fingerprint: specialization.template_contract_report_fingerprint,
                instance_report_fingerprint: specialization.report_fingerprint,
                instance_contract_report_fingerprint: instance_contract.report_fingerprint,
                instance_contract_commitment: instance_contract.commitment,
                type_argument_identities: specialization.type_argument_identities.clone(),
                const_argument_identities: specialization.const_argument_identities.clone(),
                machine_argument_contract_report_fingerprints: specialization
                    .machine_argument_contract_report_fingerprints
                    .clone(),
                machine_argument_contract_commitments,
                conformance_argument_report_fingerprints: specialization
                    .conformance_argument_report_fingerprints
                    .clone(),
                conformance_argument_commitments: specialization
                    .conformance_applications
                    .iter()
                    .map(|application| application.commitment)
                    .collect(),
            });
    }

    Ok(report)
}

fn package_key_text(identity: Option<semantic_vocabulary::PackageKeyIdentity>) -> String {
    let Some(identity) = identity else {
        return "<unbound>".to_owned();
    };
    identity
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn accepted_instance_contract_plan<'checked>(
    checked: &'checked checked_trees::CheckedTrees,
    instance: symbols::SymbolHandle,
    template_commitment: &str,
) -> Result<&'checked checked_trees::MachineContractPlan, Diagnostic> {
    exact_machine_contract_plan(
        checked,
        instance,
        &format!("accepted generic instance of `{template_commitment}`"),
    )
}

fn exact_machine_contract_plan<'checked>(
    checked: &'checked checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
    subject: &str,
) -> Result<&'checked checked_trees::MachineContractPlan, Diagnostic> {
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
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
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
    let language_semantics::ServiceReachInterface::PublishedCeiling(reach_row) =
        machine_reach.interface
    else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published service-reach ceiling"
        )));
    };
    let services = checked.facts.service_reaches.rows.services(reach_row);
    if services.is_empty() && reach_row != language_semantics::ServiceReachRowTable::EMPTY_ROW {
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
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
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
    if plan.interface != language_semantics::SynchronousInvocationInterface::PublishedCeiling {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published synchronous-invocation ceiling"
        )));
    }
    Ok(plan.published.clone())
}

fn accepted_machine_may_suspend(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
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
    let language_semantics::SuspensionInterface::PublishedMaySuspend(may_suspend) = plan.interface
    else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published suspension ceiling"
        )));
    };
    Ok(may_suspend)
}

fn accepted_machine_may_block(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
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
    let language_semantics::BlockingInterface::PublishedMayBlock(may_block) = plan.interface else {
        return Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has no published blocking ceiling"
        )));
    };
    Ok(may_block)
}

fn accepted_machine_terminates_guarantee(
    checked: &checked_trees::CheckedTrees,
    machine: symbols::SymbolHandle,
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
        language_semantics::TerminationInterface::InternalDerived => Err(Diagnostic::error(
            format!("accepted machine `{machine_name}` has no published termination interface"),
        )),
        language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::NoGuarantee,
        ) => Ok(false),
        language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::Terminates { premises },
        ) if premises.is_empty() => Ok(true),
        language_semantics::TerminationInterface::Published(
            language_semantics::TerminationGuarantee::Terminates { .. },
        ) => Err(Diagnostic::error(format!(
            "accepted machine `{machine_name}` has a progress-premised termination guarantee that cannot enter the premise-free trust row"
        ))),
    }
}

fn accepted_machine_crash_routes(
    plan: &checked_trees::MachineContractPlan,
    machine_name: &str,
) -> Result<Vec<TrustCrashRouteBucket>, Diagnostic> {
    if plan.crash.interface() != checked_trees::CrashInterface::PublishedCeiling {
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
                    checked_trees::CrashCause::Trap => TrustCrashCause::Trap,
                    checked_trees::CrashCause::Abort => TrustCrashCause::Abort,
                },
                alternative_guards: bucket
                    .alternative_guards()
                    .iter()
                    .map(|guard| match guard {
                        checked_trees::CrashRouteGuard::Truth => TrustCrashRouteGuard::Truth,
                        checked_trees::CrashRouteGuard::Predicate(predicate) => {
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
    binding: &effects::provider_plan::ProviderBinding,
) -> TrustProviderRealization {
    use effects::provider_plan::ProviderBinding;

    match binding {
        ProviderBinding::Import { evaluated } => TrustProviderRealization::Import {
            evaluated: evaluated.clone(),
        },
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
    }
}

#[cfg(test)]
mod tests;
