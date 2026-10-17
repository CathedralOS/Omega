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
        let requirement = module
            .identities
            .text(boundary.identity)
            .unwrap_or_default();
        let selected_plan = selected_provider_plans
            .iter()
            .find(|plan| plan.report_identity() == execution.provider_plan_report_identity)
            .ok_or(ProviderExecutionReplayError::Shared(
                "native execution names an unselected provider plan",
            ))?;
        required_executions.insert((
            selected_plan
                .selected_requirement_identity(requirement)
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
        validate_foreign_provider_binding(
            module
                .identities
                .text(boundary.identity)
                .unwrap_or_default(),
            foreign,
            selected_provider_plans,
            &module.identities,
        )
        .map_err(ProviderExecutionReplayError::Shared)?;
        let requirement = module
            .identities
            .text(boundary.identity)
            .unwrap_or_default();
        let selected_plan = selected_provider_plans
            .iter()
            .find(|plan| plan.report_identity() == execution.provider_plan_report_identity)
            .ok_or(ProviderExecutionReplayError::Shared(
                "native execution names an unselected provider plan",
            ))?;
        required_executions.insert((
            selected_plan
                .selected_requirement_identity(requirement)
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
    _identities: &terminal_psi::TerminalIdentityTable,
) -> Result<(), &'static str> {
    // The admitted contribution indexes the selection facts table carried
    // beside the plan's family realizations, never the module's table.
    let selected_plan = selected_provider_plans
        .iter()
        .find(|plan| {
            plan.report_identity == foreign.provider_execution.provider_plan_report_identity
        })
        .ok_or("native external call names an unselected provider plan")?;
    let binding_identities = selected_plan.family_realization_identities();
    let selected_requirement = selected_plan.selected_requirement_identity(requirement);
    if foreign.binding.requirement_identity(binding_identities) != Some(selected_requirement)
        || !foreign.binding.has_valid_entry_mechanism()
    {
        return Err("native external call binding differs from its requirement");
    }
    if let Some(contribution) = foreign.binding.same_stack_contribution() {
        validate_foreign_stack_contribution(
            requirement,
            foreign.provider_execution,
            binding_identities
                .text(contribution.requirement_identity())
                .unwrap_or_default(),
            contribution.provider_plan_report_identity(),
            contribution.provider_plan_commitment().as_bytes(),
            selected_provider_plans,
        )?;
    } else if let Some(kernel) = foreign.binding.system_call() {
        if !selected_provider_plans.iter().any(|plan| {
            plan.report_identity == kernel.provider_plan_report_identity
                && plan.report_identity == foreign.provider_execution.provider_plan_report_identity
                && *plan.plan_digest.as_bytes() == kernel.provider_plan_commitment
                && plan
                    .requirement_identities
                    .binary_search_by(|candidate| candidate.as_str().cmp(requirement))
                    .is_ok()
        }) {
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

pub(super) fn validate_provider_execution_reports(
    selected_provider_plans: &[NativeSelectedProviderPlan],
    provider_executions: &[NativeProviderExecution],
    required_executions: &BTreeSet<ProviderExecutionReportKey>,
) -> Result<(), &'static str> {
    let mut prior_plan = None;
    for plan in selected_provider_plans {
        plan.validate_external_requirement_applications()?;
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
        if !plan.requirement_identities.iter().any(|identity| {
            plan.selected_requirement_identity(identity) == execution.requirement_identity()
        }) {
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
    if contribution_provider_plan_report_identity != execution.provider_plan_report_identity {
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
    if contribution_requirement_identity
        != selected_plan.selected_requirement_identity(requirement_identity)
    {
        return Err(
            "native artifact foreign stack contribution changed its exact selected requirement",
        );
    }
    if contribution_provider_plan_commitment != *selected_plan.plan_digest().as_bytes() {
        return Err(
            "native artifact foreign stack contribution disagrees with the exact selected provider plan",
        );
    }
    Ok(())
}
