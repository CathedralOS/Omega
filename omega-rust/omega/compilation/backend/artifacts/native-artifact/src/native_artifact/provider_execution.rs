//! Replay provider executions against exact Terminal requirements and selected plans.
//!
//! Artifact owners supply their already replayed boundary-settlement and foreign-call
//! rows: the direct product uses image rows, while dynamic ELF uses object rows.
//! This owner reconstructs the required executions, checks foreign stack custody,
//! and compares the complete report set. It does not choose an image lane or grant
//! provider authority.

use std::collections::BTreeSet;

use image_emission::{ObjectBoundarySettlement, ObjectForeignCall};
use installation_evidence::ProviderExecutionEvidence;

use super::{NativeProviderExecution, NativeSelectedProviderPlan};

pub(super) fn replay_provider_executions(
    module: &terminal_psi::TerminalModule,
    boundary_settlements: &[ObjectBoundarySettlement],
    foreign_calls: &[ObjectForeignCall],
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    authority: &effects::TerminalAuthorityClosureReviewReceipt,
) -> Result<(), ProviderExecutionReplayError> {
    let mut required_executions = BTreeSet::new();
    for installed in boundary_settlements {
        let boundary = terminal_psi::find_boundary_machine(
            &module.boundary_machines,
            installed.settlement.boundary,
        )
        .ok_or(ProviderExecutionReplayError::AbsentSettlementBoundary)?;
        let image_emission::BoundaryExecutionRecord::AdmittedProvider(execution) =
            installed.settlement.execution
        else {
            continue;
        };
        if execution.direct_foreign_realization.is_some() {
            return Err(ProviderExecutionReplayError::Shared(
                "direct foreign execution retained a non-foreign settlement",
            ));
        }
        required_executions.insert((
            selected_plan_for_execution(
                selected_provider_plans,
                execution.provider_plan_report_identity,
            )
            .and_then(|plan| {
                plan.selected_requirement_identity(
                    module
                        .identities
                        .text(boundary.identity)
                        .unwrap_or_default(),
                )
                .ok_or("native artifact settlement names an absent selected requirement")
            })
            .map_err(ProviderExecutionReplayError::Shared)?
            .to_owned(),
            execution.provider_plan_report_identity,
            execution.provider_execution_report_identity,
            execution.provider_execution_report_fingerprint,
            execution.normalized_root_report_identity,
            execution.boundary_contract_report_fingerprint,
        ));
    }
    for foreign in foreign_calls {
        let (boundary_id, completion) = match foreign.owner {
            target_operations::CallSiteOwner::Operation(owner) => {
                let mut operations = module
                    .machines
                    .iter()
                    .filter(|machine| machine.id == foreign.machine)
                    .flat_map(|machine| machine.operations())
                    .filter(|operation| operation.id == owner);
                let (Some(operation), None) = (operations.next(), operations.next()) else {
                    return Err(ProviderExecutionReplayError::NonUniqueForeignOperation);
                };
                let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &operation.kind
                else {
                    return Err(ProviderExecutionReplayError::ForeignOperationIsNotBoundaryCall);
                };
                (
                    *boundary,
                    semantic_vocabulary::BoundaryCompletion::NormalReturn,
                )
            }
            target_operations::CallSiteOwner::ExternalCompletion(owner) => {
                let mut completions = module
                    .machine(foreign.machine)
                    .into_iter()
                    .flat_map(|machine| &machine.blocks)
                    .filter_map(|block| match &block.terminator {
                        terminal_psi::Terminator::ExternalCompletion {
                            edge,
                            boundary,
                            kind,
                            ..
                        } if *edge == owner => Some((*boundary, *kind)),
                        _ => None,
                    });
                let (Some((boundary, kind)), None) = (completions.next(), completions.next())
                else {
                    return Err(ProviderExecutionReplayError::NonUniqueForeignOperation);
                };
                if foreign.scalar_result.is_some()
                    || !foreign.callback_addresses.is_empty()
                    || foreign.x86_floating_control.is_some()
                    || foreign.aarch64_floating_control.is_some()
                    || foreign.binding.boundary_entry_plan.call.result.is_some()
                {
                    return Err(ProviderExecutionReplayError::Shared(
                        "native external completion retained returning-call custody",
                    ));
                }
                (
                    boundary,
                    semantic_vocabulary::BoundaryCompletion::External(kind),
                )
            }
            target_operations::CallSiteOwner::CleanupAction { .. } => {
                return Err(ProviderExecutionReplayError::MissingForeignOperationOwner);
            }
        };
        let boundary = terminal_psi::find_boundary_machine(&module.boundary_machines, boundary_id)
            .ok_or(ProviderExecutionReplayError::AbsentForeignBoundary)?;
        if boundary.completion != completion {
            return Err(ProviderExecutionReplayError::Shared(
                "native external invocation changed its completion contract",
            ));
        }
        let execution = foreign.provider_execution;
        if let Some(commitment) = execution.direct_foreign_realization {
            let direct = direct_foreign_origin(
                authority,
                module
                    .identities
                    .text(boundary.identity)
                    .unwrap_or_default(),
                commitment,
            )
            .map_err(ProviderExecutionReplayError::Shared)?;
            validate_direct_foreign_binding(
                direct,
                foreign,
                &module.identities,
                authority.target(),
            )
            .map_err(ProviderExecutionReplayError::Shared)?;
            continue;
        }
        validate_foreign_provider_binding(
            module
                .identities
                .text(boundary.identity)
                .unwrap_or_default(),
            foreign,
            selected_provider_plans,
        )
        .map_err(ProviderExecutionReplayError::Shared)?;
        required_executions.insert((
            selected_plan_for_execution(
                selected_provider_plans,
                execution.provider_plan_report_identity,
            )
            .and_then(|plan| {
                plan.selected_requirement_identity(
                    module
                        .identities
                        .text(boundary.identity)
                        .unwrap_or_default(),
                )
                .ok_or("native artifact foreign call names an absent selected requirement")
            })
            .map_err(ProviderExecutionReplayError::Shared)?
            .to_owned(),
            execution.provider_plan_report_identity,
            execution.provider_execution_report_identity,
            execution.provider_execution_report_fingerprint,
            execution.normalized_root_report_identity,
            execution.boundary_contract_report_fingerprint,
        ));
    }
    validate_provider_execution_reports(
        selected_provider_plans,
        provider_executions,
        &required_executions,
    )
    .map_err(ProviderExecutionReplayError::Shared)
}

/// Rejoin an external call's mechanism to its exact selected provider plan.
/// Kernel entries carry no opaque user-stack receipt or user-space control envelope.
pub(super) fn validate_foreign_provider_binding(
    requirement: &str,
    foreign: &ObjectForeignCall,
    selected_provider_plans: &[NativeSelectedProviderPlan],
) -> Result<(), &'static str> {
    let selected_plan = selected_plan_for_execution(
        selected_provider_plans,
        foreign.provider_execution.provider_plan_report_identity,
    )?;
    let selected_requirement = selected_plan
        .selected_requirement_identity(requirement)
        .ok_or("native external call names an absent selected requirement")?;
    // Contributions retain the original selected row in its own issuing table.
    let binding_identities = selected_plan.selection_identities();
    if foreign.binding.requirement_identity(binding_identities) != Some(selected_requirement)
        || !foreign.binding.has_valid_entry_mechanism()
    {
        return Err("native external call binding differs from its requirement");
    }
    if let Some((_, contribution)) = foreign.binding.import() {
        if !matches!(
            contribution.provider_plan_commitment(),
            task_plans::SameStackExecutionCommitment::SelectedProvider(_)
        ) {
            return Err("selected foreign call carries a direct execution commitment");
        }
        validate_foreign_stack_contribution(
            selected_requirement,
            foreign.provider_execution,
            binding_identities
                .text(contribution.requirement_identity())
                .unwrap_or_default(),
            contribution.provider_plan_report_identity(),
            contribution.provider_plan_commitment().as_bytes(),
            selected_provider_plans,
        )?;
    } else if let Some(kernel) = foreign.binding.system_call() {
        if kernel.direct_foreign_realization.is_some() {
            return Err("selected kernel entry carries direct custody");
        }
        if selected_plan.report_identity != kernel.provider_plan_report_identity
            || *selected_plan.plan_digest.as_bytes() != kernel.provider_plan_commitment
        {
            return Err("native kernel entry differs from its selected provider commitment");
        }
        if foreign.x86_floating_control.is_some()
            || foreign.aarch64_floating_control.is_some()
            || !foreign.callback_addresses.is_empty()
        {
            return Err("native kernel entry carries user-space callback or control custody");
        }
    }
    Ok(())
}

/// Structural failures shared by the two artifact products. Preserve each
/// product's existing diagnostic spelling; report and stack rejections retain
/// the same text in both.
pub(super) enum ProviderExecutionReplayError {
    AbsentSettlementBoundary,
    MissingForeignOperationOwner,
    NonUniqueForeignOperation,
    ForeignOperationIsNotBoundaryCall,
    AbsentForeignBoundary,
    Shared(&'static str),
}

impl ProviderExecutionReplayError {
    pub(super) fn direct_message(self) -> &'static str {
        match self {
            Self::AbsentSettlementBoundary => {
                "native artifact image settlement names an absent boundary requirement"
            }
            Self::MissingForeignOperationOwner => {
                "native artifact foreign provider execution has no semantic operation owner"
            }
            Self::NonUniqueForeignOperation => {
                "native artifact foreign provider execution does not rejoin one semantic operation"
            }
            Self::ForeignOperationIsNotBoundaryCall => {
                "native artifact foreign provider execution owner is not a boundary call"
            }
            Self::AbsentForeignBoundary => {
                "native artifact foreign provider execution names an absent boundary"
            }
            Self::Shared(message) => message,
        }
    }
    pub(super) fn dynamic_message(self) -> &'static str {
        match self {
            Self::AbsentSettlementBoundary => {
                "dynamic native artifact settlement names an absent boundary requirement"
            }
            Self::MissingForeignOperationOwner => {
                "dynamic native artifact foreign provider execution has no semantic operation owner"
            }
            Self::NonUniqueForeignOperation => {
                "dynamic native artifact foreign execution does not rejoin one semantic operation"
            }
            Self::ForeignOperationIsNotBoundaryCall => {
                "dynamic native artifact foreign execution owner is not a boundary call"
            }
            Self::AbsentForeignBoundary => {
                "dynamic native artifact foreign execution names an absent boundary"
            }
            Self::Shared(message) => message,
        }
    }
}

type ProviderExecutionReportKey = (String, u64, u64, u64, u64, u64);

fn selected_plan_for_execution(
    plans: &[NativeSelectedProviderPlan],
    report_identity: u64,
) -> Result<&NativeSelectedProviderPlan, &'static str> {
    let mut matches = plans
        .iter()
        .filter(|plan| plan.report_identity == report_identity);
    let selected = matches
        .next()
        .ok_or("native artifact execution names an unselected plan")?;
    if matches.next().is_some() {
        return Err("native artifact execution names competing selected plans");
    }
    Ok(selected)
}

pub(super) fn validate_provider_execution_reports(
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    required_executions: &BTreeSet<ProviderExecutionReportKey>,
) -> Result<(), &'static str> {
    let mut prior_plan = None;
    for plan in selected_provider_plans {
        if plan.report_identity == 0
            || prior_plan.is_some_and(|prior| prior >= plan.report_identity)
        {
            return Err("native artifact selected provider plans are not canonical and unique");
        }
        prior_plan = Some(plan.report_identity);
        if plan.requirement_identities.is_empty()
            || plan
                .requirement_identities
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(
                "native artifact selected provider requirements are not canonical and unique",
            );
        }
        let mut prior_application = None;
        for row in &plan.requirement_applications {
            let requirement = plan
                .selection_identities
                .text(row.requirement_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("native artifact application lost its issued requirement identity")?;
            let selected_requirement = plan
                .selection_identities
                .text(row.selected_requirement_identity)
                .filter(|identity| !identity.is_empty())
                .ok_or("native artifact application lost its original selected row")?;
            if row.provider_plan_report_identity != plan.report_identity
                || row.provider_plan_digest.as_bytes() != plan.plan_digest.as_bytes()
                || !plan.contains_requirement(requirement)
                || prior_application.is_some_and(|prior| prior >= requirement)
                || plan.family_realization_rows.iter().any(|family| {
                    (plan.selection_identities.text(family.head_identity) == Some(requirement)
                        || plan.selection_identities.text(family.member_identity)
                            == Some(requirement))
                        && plan
                            .selection_identities
                            .text(family.selected_requirement_identity)
                            != Some(selected_requirement)
                })
            {
                return Err(
                    "native artifact application changed its exact selected plan association",
                );
            }
            prior_application = Some(requirement);
        }
    }

    let mut prior_execution = None;
    let mut seen_requirements = BTreeSet::new();
    let mut reported_executions = BTreeSet::new();
    for execution in provider_executions {
        let key = (
            execution.requirement_identity(),
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
        );
        if prior_execution.is_some_and(|prior| prior >= key) {
            return Err("native artifact provider executions are not in canonical order");
        }
        prior_execution = Some(key);
        if !seen_requirements.insert(execution.requirement_identity()) {
            return Err("native artifact contains duplicate provider requirement executions");
        }
        let Some(plan) = selected_provider_plans
            .iter()
            .find(|plan| plan.report_identity == execution.provider_plan_report_identity())
        else {
            return Err("native artifact provider execution names an unselected plan");
        };
        if !plan.contains_selected_requirement(execution.requirement_identity()) {
            return Err("native artifact provider execution is absent from its selected plan");
        }
        reported_executions.insert((
            execution.requirement_identity().to_owned(),
            execution.provider_plan_report_identity(),
            execution.provider_execution_report_identity(),
            execution.provider_execution_report_fingerprint(),
            execution.normalized_root_report_identity(),
            execution.boundary_contract_report_fingerprint(),
        ));
    }
    if reported_executions != *required_executions {
        return Err("native artifact provider execution reports disagree with its image");
    }
    Ok(())
}

pub(super) fn validate_foreign_stack_contribution(
    requirement_identity: &str,
    execution: machine_code::ProviderExecutionRecord,
    contribution_requirement_identity: &str,
    contribution_provider_plan_report_identity: u64,
    contribution_provider_plan_commitment: [u8; 32],
    selected_provider_plans: &[NativeSelectedProviderPlan],
) -> Result<(), &'static str> {
    if contribution_requirement_identity != requirement_identity
        || contribution_provider_plan_report_identity != execution.provider_plan_report_identity
    {
        return Err(
            "native artifact foreign stack contribution disagrees with its semantic requirement or provider execution",
        );
    }
    let Some(selected_plan) = selected_provider_plans
        .iter()
        .find(|plan| plan.report_identity() == contribution_provider_plan_report_identity)
    else {
        return Err("native artifact foreign stack contribution names an unselected provider plan");
    };
    if contribution_provider_plan_commitment != *selected_plan.plan_digest().as_bytes() {
        return Err(
            "native artifact foreign stack contribution disagrees with the exact selected provider plan",
        );
    }
    Ok(())
}

/// Recover one retained direct origin from the exact closure, never a selected
/// row alias. Multiple exercised classifications may name the same immutable
/// origin; conflicting original records cannot substitute for it.
pub(crate) fn direct_foreign_origin<'input>(
    authority: &'input effects::TerminalAuthorityClosureReviewReceipt,
    callable: &str,
    commitment: [u8; 32],
) -> Result<&'input std::sync::Arc<effects::DirectForeignRealization>, &'static str> {
    let mut original = None;
    for leaf in authority.leaves() {
        let effects::TerminalAuthorityClosureOrigin::DirectForeign(record) = leaf.origin() else {
            continue;
        };
        if record.identity() != commitment {
            continue;
        }
        if authority.identities().text(leaf.requirement_identity()) != Some(callable)
            || record.callable_identity() != callable
            || record.target().native_target() != authority.target()
            || original.is_some_and(
                |prior: &std::sync::Arc<effects::DirectForeignRealization>| {
                    prior.as_ref() != record.as_ref()
                },
            )
        {
            return Err("direct foreign closure substituted its original callable or issuer");
        }
        original = Some(record);
    }
    original.ok_or("direct foreign execution has no retained closure origin")
}

/// Replay physical custody against the original evaluated direct binding. The
/// concrete callable and its own module index remain distinct from selection.
pub(crate) fn validate_direct_foreign_binding(
    direct: &effects::DirectForeignRealization,
    foreign: &ObjectForeignCall,
    identities: &terminal_psi::TerminalIdentityTable,
    target: target::NativeTarget,
) -> Result<(), &'static str> {
    let expected =
        target_operations::ProviderExecutionBinding::from_direct_foreign(direct.identity())
            .ok_or("direct foreign execution has an absent commitment")?;
    if foreign.provider_execution != machine_code::ProviderExecutionRecord::from(expected)
        || direct.target().native_target() != target
        || direct.boundary_entry_plan() != Some(&foreign.binding.boundary_entry_plan)
        || foreign.binding.native_target() != Some(target)
        || !foreign.binding.has_valid_entry_mechanism()
        || foreign.binding.requirement_identity(identities) != Some(direct.callable_identity())
    {
        return Err(
            "direct foreign call differs from its retained declaration or calling contract",
        );
    }
    match (direct.binding(), &foreign.binding.mechanism) {
        (
            effects::DirectForeignBinding::Import(evaluated),
            target_operations::NormalizedForeignCallMechanism::Import {
                locator,
                same_stack_contribution,
            },
        ) if locator == evaluated.locator()
            && same_stack_contribution.provider_plan_commitment()
                == task_plans::SameStackExecutionCommitment::DirectForeign(direct.identity())
            && same_stack_contribution.provider_plan_report_identity()
                == expected.provider_plan_report_identity().get()
            && identities.text(same_stack_contribution.requirement_identity())
                == Some(direct.callable_identity()) => {}
        (
            effects::DirectForeignBinding::Syscall(evaluated),
            target_operations::NormalizedForeignCallMechanism::SystemCall(kernel),
        ) if kernel.number == evaluated.number()
            && kernel.target == direct.target()
            && kernel.direct_foreign_realization == Some(direct.identity())
            && kernel.provider_plan_commitment == direct.identity()
            && kernel.provider_plan_report_identity
                == expected.provider_plan_report_identity().get()
            && kernel.requirement_identity == direct.callable_identity() =>
        {
            if foreign.x86_floating_control.is_some()
                || foreign.aarch64_floating_control.is_some()
                || !foreign.callback_addresses.is_empty()
            {
                return Err("direct kernel entry carries user-space callback or control custody");
            }
        }
        _ => return Err("direct foreign call substituted its original evaluated mechanism"),
    }
    Ok(())
}

/// Recover the exercised direct intrinsic origin, without replacing its
/// concrete declaration by a selected abstract requirement or a plan digest.
pub(crate) fn direct_compiler_intrinsic_origin<'input>(
    authority: &'input effects::TerminalAuthorityClosureReviewReceipt,
    callable: &str,
    execution: target_operations::CompilerBuiltinExecution,
) -> Result<Option<&'input std::sync::Arc<effects::DirectForeignRealization>>, &'static str> {
    let mut original = None;
    for leaf in authority.leaves() {
        let effects::TerminalAuthorityClosureOrigin::DirectForeign(record) = leaf.origin() else {
            continue;
        };
        if record.callable_identity() != callable {
            continue;
        }
        let effects::DirectForeignBinding::CompilerIntrinsic {
            execution: meaning, ..
        } = record.binding()
        else {
            continue;
        };
        if authority.identities().text(leaf.requirement_identity()) != Some(callable)
            || record.target().native_target() != authority.target()
            || leaf.mechanism() != effects::TerminalMechanismIdentity::CompilerIntrinsic(*meaning)
            || terminal_psi_to_abstract_operations::provider_admission::compiler_builtin_execution(
                *meaning, None,
            ) != Some(execution)
            || original.is_some_and(
                |prior: &std::sync::Arc<effects::DirectForeignRealization>| {
                    prior.as_ref() != record.as_ref()
                },
            )
        {
            return Err(
                "direct intrinsic closure changed its original declaration or execution issuer",
            );
        }
        original = Some(record);
    }
    Ok(original)
}
