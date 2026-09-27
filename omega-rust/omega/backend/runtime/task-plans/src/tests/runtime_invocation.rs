use super::{activation_fact, candidate, id, runtime};
use crate::{
    ExecutorPreservationAxis, ExecutorPreservationEvidence, ExecutorPreservationEvidenceId,
    TaskRuntimeInstanceId, TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptCandidate,
    TaskRuntimeInvocationReceiptId, TaskStartOperation, validate_activation_plan,
    validate_task_runtime_invocation_receipt,
};

#[test]
fn invocation_receipt_binds_the_selected_provider_operation_and_plan() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
    let activation = activation_fact(&plan);
    let instance = id(93, TaskRuntimeInstanceId::from_normalized_identity);
    let base = TaskRuntimeInvocationReceiptCandidate {
        receipt: id(94, TaskRuntimeInvocationReceiptId::from_normalized_identity),
        invocation: id(95, TaskRuntimeInvocationId::from_normalized_identity),
        runtime: runtime(),
        runtime_instance: instance,
        operation: TaskStartOperation::Start,
        provider_plan_name: activation.selected_runtime.provider_plan_name.clone(),
        requirement_identity: activation.selected_runtime.requirement_identity.clone(),
        activation_plan: plan.normalized_identity(),
        preservation: vec![ExecutorPreservationEvidence::new(
            ExecutorPreservationAxis::Cpu,
            id(96, ExecutorPreservationEvidenceId::from_normalized_identity),
        )],
    };
    let validated = validate_task_runtime_invocation_receipt(&activation, base.clone())
        .expect("exact invocation receipt");
    assert_eq!(validated.candidate(), &base);
    assert_eq!(
        validated.activation().specialization_report_fingerprint,
        activation.specialization_report_fingerprint
    );
    assert_eq!(
        validated.activation().specialization_commitment,
        activation.specialization_commitment
    );
    assert_eq!(
        validated.activation().selected_runtime,
        activation.selected_runtime
    );
    assert_eq!(
        validated.activation().activation_plan,
        plan.normalized_identity()
    );
    assert_ne!(validated.identity().normalized_identity(), 0);
    assert_eq!(
        validated.executor_selection().candidate().runtime_instance,
        instance
    );

    let mut wrong_provider = base.clone();
    wrong_provider.provider_plan_name = "OtherRuntime".into();
    assert!(
        validate_task_runtime_invocation_receipt(&activation, wrong_provider)
            .expect_err("provider drift")
            .0
            .contains("different selected provider plan")
    );

    let mut wrong_operation = base.clone();
    wrong_operation.operation = TaskStartOperation::TryStart;
    assert!(
        validate_task_runtime_invocation_receipt(&activation, wrong_operation)
            .expect_err("operation drift")
            .0
            .contains("different start operation")
    );

    let mut wrong_plan = base;
    let mut changed = candidate();
    changed.stack_plan.bytes += 16;
    wrong_plan.activation_plan = validate_activation_plan(changed)
        .expect("changed plan")
        .normalized_identity();
    assert!(
        validate_task_runtime_invocation_receipt(&activation, wrong_plan)
            .expect_err("plan drift")
            .0
            .contains("different activation plan")
    );
}
