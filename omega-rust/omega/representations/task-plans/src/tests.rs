//! Fixtures shared by the task-plan tests: a valid activation candidate,
//! a WCSU projection, a runtime and its invocation receipt.

mod activation_plans;
mod executor_selection;
mod lifecycle_ledger;
mod runtime_invocation;

use crate::{
    ActivationCarryObligations, ActivationPlanCandidate, CallingPlanId,
    CanonicalSuspensionCrossing, ExecutorPreservationAxis, ExecutorPreservationEvidence,
    ExecutorPreservationEvidenceId, MachineContractId, MachineEntryId,
    SelectedTaskRuntimeProviderFact, StackPlan, StackRepresentationId, SuspensionCrossingId,
    TaskActivationPlanFact, TaskPlanDiagnostic, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptCandidate, TaskRuntimeInvocationReceiptId,
    TaskSpecializationCommitment, TaskStackFrameId, TaskStackFrameSummary,
    TaskStackFrameValidationId, TaskStartOperation, ValidatedActivationPlan,
    ValidatedTaskRuntimeInvocationReceipt, ValueLayoutId, WcsuStackPlanProjection,
    compose_task_stack_demand, project_wcsu_stack_plan, validate_task_runtime_invocation_receipt,
    validate_task_stack_frame_summary,
};

fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, TaskPlanDiagnostic>) -> T {
    constructor(identity).expect("normalized identity")
}

fn candidate() -> ActivationPlanCandidate {
    ActivationPlanCandidate {
        machine_contract: id(1, MachineContractId::from_normalized_identity),
        entry: id(2, MachineEntryId::from_normalized_identity),
        argument_layout: id(3, ValueLayoutId::from_normalized_identity),
        terminal_outcome_layout: id(4, ValueLayoutId::from_normalized_identity),
        calling_plan: id(5, CallingPlanId::from_normalized_identity),
        stack_plan: StackPlan {
            bytes: 4096,
            alignment: 16,
            representation: id(6, StackRepresentationId::from_normalized_identity),
        },
        may_suspend: true,
        may_block: false,
        canonical_suspension_crossings: vec![CanonicalSuspensionCrossing {
            identity: SuspensionCrossingId::new(7).expect("nonzero crossing identity"),
            suspension_allowed: true,
            preserve_cpu: true,
            preserve_host_thread: false,
        }],
        carry_obligations: ActivationCarryObligations {
            preserve_cpu: true,
            preserve_host_thread: false,
        },
        cancellation_required: true,
    }
}

fn wcsu_projection(validation_identity: u64) -> WcsuStackPlanProjection {
    let root = id(30, TaskStackFrameId::from_normalized_identity);
    let frame = validate_task_stack_frame_summary(TaskStackFrameSummary {
        frame: root,
        local_bytes: 4096,
        alignment: 16,
        validation: id(
            validation_identity,
            TaskStackFrameValidationId::from_normalized_identity,
        ),
        calls: Vec::new(),
    })
    .expect("validated WCSU frame");
    let demand = compose_task_stack_demand(root, [frame]).expect("composed WCSU demand");
    project_wcsu_stack_plan(
        &demand,
        id(6, StackRepresentationId::from_normalized_identity),
    )
}

fn runtime() -> TaskRuntimeId {
    id(80, TaskRuntimeId::from_normalized_identity)
}

fn activation_fact(plan: &ValidatedActivationPlan) -> TaskActivationPlanFact {
    TaskActivationPlanFact {
        start_requirement: symbols::SymbolHandle::invalid(),
        target_machine: symbols::SymbolHandle::invalid(),
        target_entry: symbols::SymbolHandle::invalid(),
        specialization_report_fingerprint: 79,
        specialization_commitment: TaskSpecializationCommitment::from_digest([7; 32]),
        operation: TaskStartOperation::Start,
        selected_runtime: SelectedTaskRuntimeProviderFact {
            runtime: runtime(),
            provider_plan_name: "LocalTaskRuntime::satisfies::TaskRuntime".into(),
            requirement_identity: "TaskRuntime::start".into(),
        },
        plan: plan.clone(),
    }
}

fn invocation_receipt(
    plan: &ValidatedActivationPlan,
    instance: TaskRuntimeInstanceId,
    invocation: u64,
    receipt: u64,
) -> ValidatedTaskRuntimeInvocationReceipt {
    let activation = activation_fact(plan);
    validate_task_runtime_invocation_receipt(
        &activation,
        TaskRuntimeInvocationReceiptCandidate {
            receipt: id(
                receipt,
                TaskRuntimeInvocationReceiptId::from_normalized_identity,
            ),
            invocation: id(
                invocation,
                TaskRuntimeInvocationId::from_normalized_identity,
            ),
            runtime: runtime(),
            runtime_instance: instance,
            operation: TaskStartOperation::Start,
            provider_plan_name: activation.selected_runtime.provider_plan_name.clone(),
            requirement_identity: activation.selected_runtime.requirement_identity.clone(),
            activation_plan: plan.normalized_identity(),
            preservation: vec![ExecutorPreservationEvidence::new(
                ExecutorPreservationAxis::Cpu,
                id(81, ExecutorPreservationEvidenceId::from_normalized_identity),
            )],
        },
    )
    .expect("matching task runtime invocation receipt")
}
