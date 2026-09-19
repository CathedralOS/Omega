//! Fixtures shared by the task-plan tests: a valid activation candidate,
//! a WCSU projection, a runtime and its invocation receipt.

mod activation_plans;
mod argument_marshalling;
mod cancellation;
mod execution;
mod executor_selection;
mod lifecycle_ledger;
mod provider_admission;
mod runtime_invocation;
mod start_transaction;

use crate::{
    ActivationCarryObligations, ActivationPlanCandidate, CallingPlanId,
    CanonicalSuspensionCrossing, ExecutorPreservationAxis, ExecutorPreservationEvidence,
    ExecutorPreservationEvidenceId, MachineContractId, MachineEntryId, MovedTaskArguments,
    SelectedTaskRuntimeProviderFact, StackLease, StackLeaseBacking, StackPlan,
    StackRepresentationId, SuspensionCrossingId, TaskActivationPlanFact, TaskActivationPlanSet,
    TaskArgumentCustodyId, TaskArgumentLayout, TaskPlanDiagnostic, TaskRuntimeId,
    TaskRuntimeInstanceId, TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptCandidate,
    TaskRuntimeInvocationReceiptId, TaskSpecializationCommitment, TaskStackFrameId,
    TaskStackFrameSummary, TaskStackFrameValidationId, TaskStartOperation, TaskStorageLeaseId,
    TaskStorageOwnerId, TaskStorageProvenance, UnresolvedCallKind, UnresolvedCallSite,
    ValidatedActivationPlan, ValidatedTaskRuntimeInvocationReceipt, ValueLayoutId,
    WcsuStackPlanProjection, compose_task_stack_demand, establish_stack_lease,
    project_wcsu_stack_plan, validate_task_runtime_invocation_receipt,
    validate_task_stack_frame_summary, validate_wcsu_activation_plan,
};

fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, TaskPlanDiagnostic>) -> T {
    constructor(identity).expect("normalized identity")
}

/// The canonical two-field marshalling layout every `candidate()` plan
/// carries: an eight-byte field followed by a four-byte field, so the packed
/// image is 8 + 4 aligned to 16 bytes at alignment 8.
fn argument_layout() -> TaskArgumentLayout {
    TaskArgumentLayout::new(
        id(3, ValueLayoutId::from_normalized_identity),
        &[(8, 8), (4, 4)],
    )
    .expect("canonical test argument layout")
}

fn candidate() -> ActivationPlanCandidate {
    ActivationPlanCandidate {
        machine_contract: id(1, MachineContractId::from_normalized_identity),
        entry: id(2, MachineEntryId::from_normalized_identity),
        argument_layout: argument_layout(),
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
        unresolved_calls: Vec::new(),
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

/// The canonical suspension crossing every `candidate()` plan publishes.
fn canonical_crossing() -> SuspensionCrossingId {
    SuspensionCrossingId::new(7).expect("nonzero crossing identity")
}

/// An activation plan whose stack shape carries sealed whole-call-graph WCSU
/// evidence — the only kind a `StackLease` can be established against.
fn wcsu_plan(validation_identity: u64) -> ValidatedActivationPlan {
    let projection = wcsu_projection(validation_identity);
    let mut candidate = candidate();
    candidate.stack_plan = projection.stack_plan();
    validate_wcsu_activation_plan(candidate, projection).expect("WCSU-backed activation plan")
}

/// An activation plan whose projection still names unresolved call sites:
/// sealed but partial evidence whose bytes bound only the covered subgraph.
/// It elaborates and reports fine; only `establish_stack_lease` must refuse
/// it, since a partial bound is not the whole-call-graph WCSU.
fn partial_wcsu_plan(validation_identity: u64) -> ValidatedActivationPlan {
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
        unresolved_calls: vec![UnresolvedCallSite {
            frame: root,
            state: "run".into(),
            statement_index: 2,
            call_ordinal: 0,
            kind: UnresolvedCallKind::UnresolvedTarget,
        }],
    })
    .expect("validated WCSU frame");
    let demand = compose_task_stack_demand(root, [frame]).expect("composed WCSU demand");
    let projection = project_wcsu_stack_plan(
        &demand,
        id(6, StackRepresentationId::from_normalized_identity),
    );
    assert!(!projection.is_exact());
    let mut candidate = candidate();
    candidate.stack_plan = projection.stack_plan();
    validate_wcsu_activation_plan(candidate, projection)
        .expect("partial-WCSU-backed activation plan")
}

/// A stack lease backed by exactly the plan's demanded shape.
fn stack_lease(plan: &ValidatedActivationPlan, owner: u64, lease: u64) -> StackLease {
    establish_stack_lease(
        plan,
        StackLeaseBacking {
            provenance: TaskStorageProvenance {
                owner: id(owner, TaskStorageOwnerId::from_normalized_identity),
                lease: id(lease, TaskStorageLeaseId::from_normalized_identity),
            },
            backing: plan.candidate().stack_plan,
        },
    )
    .expect("backing satisfies the plan")
}

/// Marshal one deterministic bundle under `layout`: each field filled with
/// its own byte pattern (`0xA0 + index`) so byte-exact conservation is
/// observable on the rejected and accepted paths.
fn marshal_arguments(layout: &TaskArgumentLayout, custody: u64) -> MovedTaskArguments {
    let arguments: Vec<Vec<u8>> = layout
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| vec![0xA0u8 + index as u8; field.bytes as usize])
        .collect();
    let views: Vec<&[u8]> = arguments.iter().map(Vec::as_slice).collect();
    MovedTaskArguments::marshal(
        layout,
        &views,
        id(custody, TaskArgumentCustodyId::from_normalized_identity),
    )
    .expect("test arguments marshal under the layout")
}

fn moved_arguments(plan: &ValidatedActivationPlan, custody: u64) -> MovedTaskArguments {
    marshal_arguments(&plan.candidate().argument_layout, custody)
}

fn activation_fact(plan: &ValidatedActivationPlan) -> TaskActivationPlanFact {
    activation_fact_for(plan, TaskStartOperation::Start)
}

fn activation_fact_for(
    plan: &ValidatedActivationPlan,
    operation: TaskStartOperation,
) -> TaskActivationPlanFact {
    TaskActivationPlanFact {
        start_requirement: symbols::SymbolHandle::invalid(),
        target_machine: symbols::SymbolHandle::invalid(),
        target_entry: symbols::SymbolHandle::invalid(),
        specialization_report_fingerprint: 79,
        specialization_commitment: TaskSpecializationCommitment::from_digest([7; 32]),
        operation,
        selected_runtime: SelectedTaskRuntimeProviderFact {
            runtime: runtime(),
            provider_plan_name: "LocalTaskRuntime::satisfies::TaskRuntime".into(),
            requirement_identity: "TaskRuntime::start".into(),
        },
        plan: plan.clone(),
    }
}

fn activation_set(plan: &ValidatedActivationPlan) -> TaskActivationPlanSet {
    TaskActivationPlanSet {
        activations: vec![activation_fact(plan)],
    }
}

fn receipt_candidate(
    plan: &ValidatedActivationPlan,
    instance: TaskRuntimeInstanceId,
    invocation: u64,
    receipt: u64,
    operation: TaskStartOperation,
) -> TaskRuntimeInvocationReceiptCandidate {
    let activation = activation_fact_for(plan, operation);
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
        operation,
        provider_plan_name: activation.selected_runtime.provider_plan_name.clone(),
        requirement_identity: activation.selected_runtime.requirement_identity.clone(),
        activation_plan: plan.normalized_identity(),
        preservation: vec![ExecutorPreservationEvidence::new(
            ExecutorPreservationAxis::Cpu,
            id(81, ExecutorPreservationEvidenceId::from_normalized_identity),
        )],
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
        receipt_candidate(
            plan,
            instance,
            invocation,
            receipt,
            TaskStartOperation::Start,
        ),
    )
    .expect("matching task runtime invocation receipt")
}
