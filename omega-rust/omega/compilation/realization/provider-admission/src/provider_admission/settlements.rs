/*
Provider-execution boundary settlement: derive each demanded leaf's terminal
mechanism, admit each exact boundary realization, and publish executions in
canonical order.

Preparation settles one execution per selected row, whose retained family and
specialization relations may serve several original call contracts.
`settle_boundary` rejoins each settlement's exact selected plan, checks its
declaration contract on every matching boundary (including dormant family
members), and emits an `AdmittedBoundarySettlement` for each boundary the entry
actually demands.
*/

/*
We keep physical settlement separated by transport role:
- `rejoin_normalized_foreign_call`: joins same-stack custody, external calling
  application, and either a table offset (`VtableField` / `TableFunction`) or
  an evaluated import locator (`Import`).
- `rejoin_component_call`: joins an already checked cross-artifact call,
  verifying the producer's `AdmittedNativeCallableAuthority`, export slot
  commitment, ABI application, and same-stack contribution without inferring
  any `ForeignBinding` row or callback body composition from the locator.
- `rejoin_system_call`: joins a direct kernel entry to its exact selected
  provider and evaluated target syscall calling policy without a foreign
  same-stack reserve.
*/

mod source_imports;

use crate::AdmittedBoundarySettlement;
use crate::provider_admission::terminal_authority_policy::TerminalAuthorityPolicyRow;
use crate::provider_admission::{
    AdmittedTerminalMechanism, NativeBoundaryRealization, NativeProviderSettlement,
    NativeRealizationInput, ProviderAdmissionRequest,
};
use calling_conventions::{CallSignature, CallingPolicy, ExternalBindingKind, ExternalBindingRow};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use installation_evidence::ProviderExecutionEvidence;
use source_imports::derive_source_evaluated_mechanisms;
use target::NativeTarget;
use target_operations::{
    NormalizedForeignCallBinding, NormalizedForeignCallMechanism, NormalizedSystemCallBinding,
};

pub(crate) fn settle_provider_executions<'request>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'_, 'request>,
    receiver: terminal_psi::TerminalPsiIdentity,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        Vec<&'request dyn ProviderExecutionEvidence>,
        Vec<AdmittedTerminalMechanism>,
        Vec<TerminalAuthorityPolicyRow>,
    ),
    Vec<Diagnostic>,
> {
    let (mechanisms, cohort_rows) = derive_source_evaluated_mechanisms(
        input,
        request.selected_provider_plans,
        request.external_binding_rows,
        request.native_callbacks,
    )?;
    let mut admitted = Vec::with_capacity(request.settlements.len());
    let mut provider_executions = Vec::with_capacity(request.settlements.len());

    for settlement in request.settlements {
        let (admitted_settlement, provider_execution) =
            settle_boundary(input, request, settlement, receiver)?;
        admitted.extend(admitted_settlement);
        provider_executions.push(provider_execution);
    }

    provider_executions.sort_by(|left, right| {
        (
            left.requirement_identity(),
            left.provider_plan_report_identity(),
            left.provider_execution_report_identity(),
        )
            .cmp(&(
                right.requirement_identity(),
                right.provider_plan_report_identity(),
                right.provider_execution_report_identity(),
            ))
    });
    Ok((admitted, provider_executions, mechanisms, cohort_rows))
}

fn settle_boundary<'request>(
    input: &NativeRealizationInput,
    request: &ProviderAdmissionRequest<'_, 'request>,
    settlement: &NativeProviderSettlement<'request>,
    receiver: terminal_psi::TerminalPsiIdentity,
) -> Result<
    (
        Vec<AdmittedBoundarySettlement<'request>>,
        &'request dyn ProviderExecutionEvidence,
    ),
    Vec<Diagnostic>,
> {
    let evidence = settlement.provider_execution;
    let requirement = evidence.requirement_identity();
    let selected_plan = selected_plan_from_exact_evidence(
        request.selected_provider_plans,
        evidence.provider_plan_report_identity(),
        settlement.provider_plan,
        requirement,
    )?;
    if !selected_plan
        .rows
        .iter()
        .any(|row| selected_plan.identities.text(row.requirement_identity) == requirement)
    {
        return Err(vec![Diagnostic::error(format!(
            "native provider execution for `{requirement}` is absent from selected plan `{}`",
            selected_plan.name
        ))]);
    }
    let realize = |abi: &semantic_vocabulary::CallingAbiIdentity| -> Result<
        target_operations::BoundarySettlementRealization,
        Vec<Diagnostic>,
    > {
        Ok(match settlement.realization {
            NativeBoundaryRealization::SystemCall => {
                target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                    rejoin_system_call(
                        selected_plan,
                        request.external_binding_rows,
                        requirement,
                        request.target(),
                        abi,
                    )?,
                )
            }
            NativeBoundaryRealization::NormalizedForeignCall(same_stack) => {
                target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                    rejoin_normalized_foreign_call(
                        selected_plan,
                        request.external_binding_rows,
                        same_stack,
                        evidence.provider_plan_report_identity(),
                        requirement,
                        request.selected_provider_plans.identities(),
                        request.target(),
                        abi,
                    )?,
                )
            }
            NativeBoundaryRealization::Component {
                call,
                authority,
                locator,
                same_stack,
            } => target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                rejoin_component_call(
                    &call,
                    authority,
                    locator,
                    same_stack,
                    selected_plan,
                    requirement,
                    request.selected_provider_plans.identities(),
                    receiver,
                    request.profile,
                    abi,
                )?,
            ),
            NativeBoundaryRealization::Builtin(realization) => {
                target_operations::BoundarySettlementRealization::Builtin(realization)
            }
        })
    };
    let matching_boundaries = input
        .module()
        .boundary_machines
        .iter()
        .filter(|boundary| {
            (!matches!(settlement.realization, NativeBoundaryRealization::Component { call, .. } if boundary.id != call.selection().receiving_boundary()))
                && request
                    .selected_provider_plans
                    .selected_row_matches_requirement(
                        input
                            .module()
                            .identities
                            .text(boundary.identity)
                            .unwrap_or_default(),
                        requirement,
                        evidence.provider_plan_report_identity(),
                    )
        })
        .collect::<Vec<_>>();
    if matching_boundaries.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "native provider execution cites absent requirement `{requirement}`"
        ))]);
    }
    let mut admitted = Vec::with_capacity(matching_boundaries.len());
    for boundary in matching_boundaries {
        if !super::compiler_builtins::boundary_contract_matches_selected_schema(
            boundary,
            selected_plan,
            requirement,
        ) {
            return Err(vec![Diagnostic::error(format!(
                "provider completion for `{requirement}` disagrees with its exact retained requirement contract"
            ))]);
        }
        if input.plan().boundary_machine(boundary.id).is_none() {
            continue;
        }
        let realization =
            match settlement.realization {
                NativeBoundaryRealization::Builtin(realization) => {
                    target_operations::BoundarySettlementRealization::Builtin(realization)
                }
                _ => realize(&input.selected_calling_abi_identity(boundary.id).map_err(
                    |reason| {
                        vec![Diagnostic::error(format!(
                            "provider settlement for `{requirement}` at {:?}: {reason}",
                            boundary.id
                        ))]
                    },
                )?)?,
            };
        if let target_operations::BoundarySettlementRealization::Builtin(builtin) = &realization {
            let completion = match builtin {
                target_operations::BoundaryRealization::StaticInterrupt { .. } => {
                    return Err(vec![Diagnostic::error(
                        "provider execution cannot replace installed interrupt admission",
                    )]);
                }
                target_operations::BoundaryRealization::AccessLifecycle { .. }
                | target_operations::BoundaryRealization::EmptyRunRelease => {
                    return Err(vec![Diagnostic::error(
                        "provider execution cannot replace a private access lifecycle",
                    )]);
                }
                target_operations::BoundaryRealization::HostedExitProcessI32(_) => {
                    semantic_vocabulary::BoundaryCompletion::External(
                        semantic_vocabulary::ExternalCompletionKind::ProcessExit,
                    )
                }
                target_operations::BoundaryRealization::ProgramImageRetainedSuffixStart
                | target_operations::BoundaryRealization::MetadataOnlyPort(_)
                | target_operations::BoundaryRealization::DirectPortReadU8(_)
                | target_operations::BoundaryRealization::LinuxWriteLine(_)
                | target_operations::BoundaryRealization::HostedReadByte(_)
                | target_operations::BoundaryRealization::HostedWriteByteI32(_)
                | target_operations::BoundaryRealization::ClaimCompletionOnly(_)
                | target_operations::BoundaryRealization::AccessorScalarAccess(_)
                | target_operations::BoundaryRealization::NearestIeeeFloatArithmetic(_) => {
                    semantic_vocabulary::BoundaryCompletion::NormalReturn
                }
            };
            if boundary.completion != completion {
                return Err(vec![Diagnostic::error(format!(
                    "provider builtin for `{requirement}` contradicts its retained completion contract"
                ))]);
            }
        }
        admitted.push(AdmittedBoundarySettlement {
            boundary: boundary.id,
            execution: crate::AdmittedBoundaryExecution::Provider(evidence),
            realization: realization.clone(),
        });
    }
    Ok((admitted, evidence))
}

fn external_calling_application<'row>(
    row: &'row calling_conventions::ExternalBindingRow,
    plan: &effects::provider_plan::ProviderPlan,
    abi: &semantic_vocabulary::CallingAbiIdentity,
) -> Result<&'row calling_conventions::BoundaryCallingApplication, Vec<Diagnostic>> {
    let application = row.calling_application(abi).ok_or_else(|| {
        vec![Diagnostic::error(
            "external binding lost its exact declaration ABI application",
        )]
    })?;
    let dispatch_only = u32::from(matches!(
        row.binding,
        calling_conventions::ExternalBindingKind::TableFunction { .. }
    ));
    if application.dispatch_only_parameter_count != dispatch_only {
        return Err(vec![Diagnostic::error(
            "external calling application has another dispatch projection",
        )]);
    }
    let declared = plan
        .schema
        .calling_applications
        .iter()
        .any(|application| application.abi.requirement_identity == abi.requirement_identity);
    if declared {
        let expected = plan.schema.calling_application(abi).ok_or_else(|| {
            vec![Diagnostic::error(
                "selected external schema has no such exact declaration ABI application",
            )]
        })?;
        if application.commitment != expected.commitment.as_bytes()
            || application.report_fingerprint != expected.report_fingerprint
        {
            return Err(vec![Diagnostic::error(
                "external ABI application differs from its independently selected calling policy",
            )]);
        }
    }
    Ok(application)
}

fn selected_plan_from_exact_evidence<'facts>(
    selected: &'facts effects::SelectedProviderPlanFacts,
    report_identity: u64,
    exact_plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
) -> Result<&'facts effects::provider_plan::ProviderPlan, Vec<Diagnostic>> {
    selected
        .plan_by_exact_evidence(report_identity, exact_plan)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "native provider execution for `{requirement}` does not carry exact evidence for selected plan {report_identity:#018x}"
            ))]
        })
}

#[allow(clippy::too_many_arguments)]
fn rejoin_component_call(
    call: &crate::ComponentCallFacts<'_>,
    authority: &super::AdmittedNativeCallableAuthority,
    locator: &target::NormalizedComponentLocator,
    same_stack: &task_plans::AdmittedSameStackContribution,
    selected_plan: &effects::provider_plan::ProviderPlan,
    requirement: &str,
    identities: &terminal_psi::TerminalIdentityTable,
    receiver: terminal_psi::TerminalPsiIdentity,
    profile: target::TargetProfile,
    abi: &semantic_vocabulary::CallingAbiIdentity,
) -> Result<target_operations::NormalizedForeignCallBinding, Vec<Diagnostic>> {
    let selection = call.selection();
    let application = call.application();
    let selected_commitment = task_plans::SameStackProviderPlanCommitment::from_digest(
        *selected_plan.identity_digest().as_bytes(),
    );
    if authority.subject() != selection.provider()
        || authority.entry() != selection.candidate()
        || authority.review().terminal_artifact_identity()
            != *call.component().artifact().manifest().identity().as_bytes()
        || authority.review().target() != profile.native_target()
        || selection.receiver() != &terminal_codec::TerminalExecutionIdentity::portable(receiver)
        || selection.provider() != call.component().subject()
        || call.plan_digest() != &selected_plan.identity_digest()
        || &application.abi != abi
        || application.target != profile.native_target()
        || locator.target() != profile
        || locator.slot_commitment()
            != &target::component_export_slot(
                selection.provider().vocabulary_marker.get(),
                selection.provider().program_fingerprint.as_bytes(),
                &application.commitment,
            )
        || application.recomputed_commitment().ok() != Some(application.commitment)
        || same_stack.provider_plan_report_identity() != selected_plan.report_fingerprint()
        || same_stack.provider_plan_commitment() != selected_commitment
        || identities.index_of(requirement) != Some(same_stack.requirement_identity())
    {
        return Err(vec![Diagnostic::error(format!(
            "component realization for `{requirement}` lost its exact artifact, selected plan, ABI application or same-stack custody",
        ))]);
    }
    Ok(target_operations::NormalizedForeignCallBinding {
        boundary_entry_plan: application.plan.plan().clone(),
        mechanism: target_operations::NormalizedForeignCallMechanism::Component {
            locator: locator.clone(),
            provider: selection.provider(),
            candidate: selection.candidate(),
            application_commitment: application.commitment,
            same_stack_contribution: same_stack.clone(),
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn rejoin_normalized_foreign_call(
    selected_plan: &effects::provider_plan::ProviderPlan,
    external_binding_rows: &[calling_conventions::ExternalBindingRow],
    same_stack: &task_plans::AdmittedSameStackContribution,
    provider_plan_report_identity: u64,
    requirement: &str,
    identities: &terminal_psi::TerminalIdentityTable,
    target: target::NativeTarget,
    abi: &semantic_vocabulary::CallingAbiIdentity,
) -> Result<NormalizedForeignCallBinding, Vec<Diagnostic>> {
    let selected_commitment = task_plans::SameStackProviderPlanCommitment::from_digest(
        *selected_plan.identity_digest().as_bytes(),
    );
    if same_stack.provider_plan_report_identity() != provider_plan_report_identity
        || same_stack.provider_plan_commitment() != selected_commitment
        || identities.index_of(requirement) != Some(same_stack.requirement_identity())
    {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not carry same-stack custody for the exact selected provider row"
        ))]);
    }
    let selected_rows = selected_plan
        .rows
        .iter()
        .filter(|row| selected_plan.identities.text(row.requirement_identity) == requirement)
        .collect::<Vec<_>>();
    let [selected_row] = selected_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` resolves to {} selected provider rows",
            selected_rows.len()
        ))]);
    };
    let external_rows = external_binding_rows
        .iter()
        .filter(|row| row.requirement_identity == requirement)
        .collect::<Vec<_>>();
    let [external] = external_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` resolves to {} retained external-binding rows",
            external_rows.len()
        ))]);
    };
    let application = external_calling_application(external, selected_plan, abi)?;
    if let (Some(boundary_entry_plan), selected, retained) = (
        Some(application.plan.plan()),
        &selected_row.binding,
        &external.binding,
    ) {
        let table = match (selected, retained) {
            (
                effects::provider_plan::ProviderBinding::VtableField { field, .. },
                calling_conventions::ExternalBindingKind::VtableField {
                    field: retained,
                    byte_offset,
                },
            ) if field == retained => Some((*byte_offset, false)),
            (
                effects::provider_plan::ProviderBinding::TableFunction { field, .. },
                calling_conventions::ExternalBindingKind::TableFunction {
                    field: retained,
                    byte_offset,
                },
            ) if field == retained => Some((*byte_offset, true)),
            _ => None,
        };
        if let Some((byte_offset, dispatch_only)) = table {
            let profile = target::TargetProfile::from_canonical_target_name(&selected_plan.target)
                .map_err(|error| vec![error])?;
            if profile.native_target() != target {
                return Err(vec![Diagnostic::error(
                    "table call names a different native target",
                )]);
            }
            return Ok(NormalizedForeignCallBinding {
                boundary_entry_plan: boundary_entry_plan.clone(),
                mechanism: target_operations::NormalizedForeignCallMechanism::Table {
                    target: profile,
                    byte_offset,
                    dispatch_only,
                    same_stack_contribution: same_stack.clone(),
                },
            });
        }
    }
    let (
        effects::provider_plan::ProviderBinding::Import {
            evaluated: selected_import,
        },
        calling_conventions::ExternalBindingKind::Import {
            locator: retained_locator,
        },
        Some(boundary_entry_plan),
    ) = (
        &selected_row.binding,
        &external.binding,
        Some(application.plan.plan()),
    )
    else {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not rejoin an evaluated import row with a calling plan"
        ))]);
    };
    let selected_locator = selected_import.locator();
    if selected_locator != retained_locator || retained_locator.target().native_target() != target {
        return Err(vec![Diagnostic::error(format!(
            "normalized foreign realization for `{requirement}` does not match the exact selected locator and native target"
        ))]);
    }
    Ok(NormalizedForeignCallBinding {
        boundary_entry_plan: boundary_entry_plan.clone(),
        mechanism: target_operations::NormalizedForeignCallMechanism::Import {
            locator: retained_locator.clone(),
            same_stack_contribution: same_stack.clone(),
        },
    })
}

fn rejoin_system_call(
    selected_plan: &ProviderPlan,
    external_rows: &[ExternalBindingRow],
    requirement: &str,
    target: NativeTarget,
    abi: &semantic_vocabulary::CallingAbiIdentity,
) -> Result<NormalizedForeignCallBinding, Vec<Diagnostic>> {
    let invalid = |reason| {
        vec![Diagnostic::error(format!(
            "syscall realization for `{requirement}` {reason}"
        ))]
    };
    let profile = target::TargetProfile::from_canonical_target_name(&selected_plan.target)
        .map_err(|error| vec![error])?;
    if profile.native_target() != target {
        return Err(invalid("does not match the selected native target"));
    }
    let policy = CallingPolicy::native_syscall_for_target(target)
        .ok_or_else(|| invalid("has no supported kernel entry policy"))?;
    let mut selected = selected_plan
        .rows
        .iter()
        .filter(|row| selected_plan.identities.text(row.requirement_identity) == requirement);
    let selected_row = selected
        .next()
        .ok_or_else(|| invalid("has no selected row"))?;
    if selected.next().is_some() {
        return Err(invalid("has duplicate selected rows"));
    }
    let ProviderBinding::Syscall { number } = selected_row.binding else {
        return Err(invalid("does not select a syscall binding"));
    };
    if number < 0 {
        return Err(invalid("has a negative syscall number"));
    }
    let mut retained = external_rows
        .iter()
        .filter(|row| row.requirement_identity == requirement);
    let external = retained
        .next()
        .ok_or_else(|| invalid("has no retained external row"))?;
    if retained.next().is_some() {
        return Err(invalid("has duplicate retained external rows"));
    }
    if external.binding != (ExternalBindingKind::Syscall { number })
        || external.target_name != selected_plan.target
        || external.trait_name
            != selected_plan
                .schema
                .identity_text(selected_plan.schema.declaration.trait_name)
        || external.method != selected_row.method
        || external.provider_type != selected_plan.provider_type
    {
        return Err(invalid(&format!(
            "does not rejoin the exact selected external row: retained binding {:?}, target `{}`, trait `{}`, method `{}`, provider `{}`; selected binding {:?}, target `{}`, trait `{}`, method `{}`, provider `{}`",
            external.binding,
            external.target_name,
            external.trait_name,
            external.method,
            external.provider_type,
            selected_row.binding,
            selected_plan.target,
            selected_plan
                .schema
                .identity_text(selected_plan.schema.declaration.trait_name),
            selected_row.method,
            selected_plan.provider_type,
        )));
    }
    let entry = external_calling_application(external, selected_plan, abi)?
        .plan
        .plan();
    if entry.call.policy != policy {
        return Err(invalid(
            "does not retain the target's syscall calling policy",
        ));
    }
    let signature = CallSignature {
        parameters: entry
            .call
            .parameters
            .iter()
            .map(|parameter| parameter.shape)
            .collect(),
        result: entry.call.result.as_ref().map(|result| result.shape),
    };
    let expected = calling_conventions::evaluate_call_plan(policy, &signature)
        .map_err(|_| invalid("has an unsupported syscall signature"))?;
    if entry.call != expected {
        return Err(invalid("does not retain the exact kernel ABI"));
    }
    calling_conventions::validate_boundary_entry_plan(entry.clone(), &signature)
        .map_err(|_| invalid("has a noncanonical syscall calling plan"))?;
    Ok(NormalizedForeignCallBinding {
        boundary_entry_plan: entry.clone(),
        mechanism: NormalizedForeignCallMechanism::SystemCall(NormalizedSystemCallBinding {
            target: profile,
            number,
            requirement_identity: requirement.to_owned(),
            provider_plan_report_identity: selected_plan.report_fingerprint(),
            provider_plan_commitment: *selected_plan.identity_digest().as_bytes(),
        }),
    })
}
