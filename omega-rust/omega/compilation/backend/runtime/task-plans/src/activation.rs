//! Close portable task demands under one selected runtime and target.
//! Psi has already established source contracts, crossings and call identities.
//! This join owns provider coverage, storage geometry and whole-stack composition.
mod runtime_requirements;
mod storage;

use crate::*;
use diagnostics::Diagnostic;
use semantic_vocabulary::Fnv1a;
use storage::TaskStorage;
use terminal_psi::*;

/// The same source-free operation serves checking-only projection and native
/// artifact admission. No source symbol or checked-tree arena is an input.
pub fn realize_task_activations(
    demands: &TerminalTaskActivationDemands,
    identities: &TerminalIdentityTable,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    profile: target::TargetProfile,
) -> Result<TaskActivationPlanSet, Vec<Diagnostic>> {
    if demands.demands.is_empty() {
        return Ok(TaskActivationPlanSet::default());
    }
    let storage = TaskStorage::new(demands, profile)?;
    let target = profile.native_target();
    let mut activations = Vec::new();
    let mut plan_identities = TerminalIdentityTableBuilder::default();
    for demand in &demands.demands {
        let selected_runtime = runtime_requirements::selected_task_runtime_provider(
            identities,
            selected_provider_plans,
            demand,
        )?;
        let arguments = demand
            .signature_arguments
            .iter()
            .map(|value| {
                storage
                    .value(value.value_type)
                    .map(|shape| (value.normalized_identity, shape))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let argument_identity = signature_layout_identity(identities, &arguments)?;
        let argument_layout = TaskArgumentLayout::new(
            normalized_id(argument_identity, ValueLayoutId::from_normalized_identity)?,
            &arguments
                .iter()
                .map(|(_, shape)| (shape.0, shape.1))
                .collect::<Vec<_>>(),
        )
        .map_err(plan_diagnostic)?;
        let outcome = storage.value(demand.signature_outcome.value_type)?;
        let outcome_identity = signature_layout_identity(
            identities,
            &[(demand.signature_outcome.normalized_identity, outcome)],
        )?;
        let mut calling = Fnv1a::new();
        calling.bytes(&target.wire_identity_bytes());
        calling.u64(demand.entry_report_fingerprint);
        calling.u64(argument_identity);
        calling.u64(outcome_identity);
        let frames = demand
            .frames
            .iter()
            .map(|frame| storage.frame(frame, identities))
            .collect::<Result<Vec<_>, _>>()?;
        let stack_demand = compose_task_stack_demand(
            normalized_id(
                demand.root_frame,
                TaskStackFrameId::from_normalized_identity,
            )?,
            frames,
        )
        .map_err(plan_diagnostic)?;
        let mut representation = Fnv1a::new();
        representation.byte(0x53);
        representation.string_terminated("fixed-nonmoving-stack-v1");
        representation.bytes(&target.wire_identity_bytes());
        let projection = project_wcsu_stack_plan(
            &stack_demand,
            normalized_id(
                representation.finish_or(1),
                StackRepresentationId::from_normalized_identity,
            )?,
        );
        let crossings = demand
            .crossings
            .iter()
            .map(crossing)
            .collect::<Result<Vec<_>, _>>()?;
        let plan = validate_wcsu_activation_plan(
            ActivationPlanCandidate {
                machine_contract: normalized_id(
                    demand.contract_report_fingerprint,
                    MachineContractId::from_normalized_identity,
                )?,
                entry: normalized_id(
                    demand.entry_report_fingerprint,
                    MachineEntryId::from_normalized_identity,
                )?,
                argument_layout,
                terminal_outcome_layout: normalized_id(
                    outcome_identity,
                    ValueLayoutId::from_normalized_identity,
                )?,
                calling_plan: normalized_id(
                    calling.finish_or(1),
                    CallingPlanId::from_normalized_identity,
                )?,
                stack_plan: projection.stack_plan(),
                may_suspend: demand.may_suspend,
                may_block: demand.may_block,
                canonical_suspension_crossings: crossings,
                carry_obligations: ActivationCarryObligations {
                    preserve_cpu: demand.carry_obligations.preserve_cpu,
                    preserve_host_thread: demand.carry_obligations.preserve_host_thread,
                },
                cancellation_required: demand.cancellation_required,
            },
            projection,
        )
        .map_err(plan_diagnostic)?;
        activations.push(TaskActivationPlanFact {
            start_requirement: plan_identities.intern(identity_text(
                identities,
                demand.requirement_callable_identity,
            )?),
            target_machine: plan_identities
                .intern(identity_text(identities, demand.machine_callable_identity)?),
            target_entry: plan_identities.intern(identity_text(identities, demand.entry)?),
            specialization_report_fingerprint: demand.specialization_report_fingerprint,
            specialization_commitment: TaskSpecializationCommitment::from_digest(
                demand.specialization_commitment,
            ),
            operation: match demand.operation {
                TerminalTaskStartOperation::Start => TaskStartOperation::Start,
                TerminalTaskStartOperation::TryStart => TaskStartOperation::TryStart,
            },
            selected_runtime,
            plan,
        });
    }
    Ok(TaskActivationPlanSet {
        identities: plan_identities.build(),
        activations,
    })
}

fn crossing(
    crossing: &TerminalTaskSuspensionCrossing,
) -> Result<CanonicalSuspensionCrossing, Vec<Diagnostic>> {
    let live_carry = crossing
        .live_carry
        .iter()
        .map(|live| {
            Ok(LiveCarryDemand {
                place: normalized_id(live.place, LiveCarryPlaceId::from_normalized_identity)?,
                ty: normalized_id(live.value_type, LiveCarryTypeId::from_normalized_identity)?,
                storage: match live.storage {
                    TerminalTaskCarryStorage::Persistent => LiveCarryStorage::Persistent,
                    TerminalTaskCarryStorage::Parameter => LiveCarryStorage::Parameter,
                    TerminalTaskCarryStorage::Local => LiveCarryStorage::Local,
                    TerminalTaskCarryStorage::CallArgument => LiveCarryStorage::CallArgument,
                },
                claims: live
                    .claims
                    .iter()
                    .map(|claim| {
                        ClaimId::new(*claim).ok_or_else(|| {
                            vec![Diagnostic::error("task carry claim has zero identity")]
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                effective: live.effective,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(CanonicalSuspensionCrossing {
        identity: SuspensionCrossingId::new(crossing.identity)
            .ok_or_else(|| vec![Diagnostic::error("task crossing has zero identity")])?,
        suspension_allowed: crossing.suspension_allowed,
        preserve_cpu: crossing.preserve_cpu,
        preserve_host_thread: crossing.preserve_host_thread,
        live_carry,
    })
}

fn signature_layout_identity(
    identities: &TerminalIdentityTable,
    values: &[(TerminalIdentityIndex, (u64, u64))],
) -> Result<u64, Vec<Diagnostic>> {
    let mut hash = Fnv1a::new();
    hash.byte(0x51);
    for (identity, (size, alignment)) in values {
        hash.string_terminated(identity_text(identities, *identity)?);
        hash.u64(*size);
        hash.u64(*alignment);
        hash.byte(0xff);
    }
    Ok(hash.finish_or(1))
}

pub(super) fn identity_text(
    identities: &TerminalIdentityTable,
    identity: TerminalIdentityIndex,
) -> Result<&str, Vec<Diagnostic>> {
    identities
        .text(identity)
        .ok_or_else(|| vec![Diagnostic::error("task demand has an unresolved identity")])
}

pub(super) fn normalized_id<T>(
    identity: u64,
    constructor: impl FnOnce(u64) -> Result<T, TaskPlanDiagnostic>,
) -> Result<T, Vec<Diagnostic>> {
    constructor(identity).map_err(plan_diagnostic)
}
pub(super) fn plan_diagnostic(error: TaskPlanDiagnostic) -> Vec<Diagnostic> {
    vec![Diagnostic::error(error.to_string())]
}
