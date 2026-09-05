use task_plans::{
    ActivationCarryObligations, ActivationPlanCandidate, CallingPlanId,
    CanonicalSuspensionCrossing, ExecutorPreservationAxis, ExecutorPreservationEvidence,
    ExecutorPreservationEvidenceId, ExecutorSelectionCandidate, MachineContractId, MachineEntryId,
    StackPlan, StackRepresentationId, SuspensionCrossingId, TaskPlanDiagnostic, TaskRuntimeId,
    TaskRuntimeInstanceId, ValueLayoutId, validate_activation_plan, validate_executor_selection,
};

fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, TaskPlanDiagnostic>) -> T {
    constructor(identity).expect("normalized test identity")
}

#[test]
fn selected_executor_with_incompatible_affinity_evidence_rejects() {
    let plan = validate_activation_plan(ActivationPlanCandidate {
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
    })
    .expect("valid provider-independent activation plan");

    let diagnostic = validate_executor_selection(
        &plan,
        ExecutorSelectionCandidate {
            runtime: id(8, TaskRuntimeId::from_normalized_identity),
            runtime_instance: id(9, TaskRuntimeInstanceId::from_normalized_identity),
            preservation: vec![ExecutorPreservationEvidence::new(
                ExecutorPreservationAxis::HostThread,
                id(10, ExecutorPreservationEvidenceId::from_normalized_identity),
            )],
        },
    )
    .expect_err("host-thread evidence cannot satisfy a CPU-affine activation");

    assert!(
        diagnostic
            .0
            .contains("selected executor does not establish CPU preservation")
    );
}
