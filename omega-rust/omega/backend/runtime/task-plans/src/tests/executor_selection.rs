use super::{candidate, id, runtime};
use crate::{
    ExecutorPreservationAxis, ExecutorPreservationEvidence, ExecutorPreservationEvidenceId,
    ExecutorSelectionCandidate, TaskRuntimeInstanceId, validate_activation_plan,
    validate_executor_selection,
};

#[test]
fn incompatible_affinity_executor_selection_rejects() {
    let plan = validate_activation_plan(candidate()).expect("activation plan");
    let diagnostic = validate_executor_selection(
        &plan,
        ExecutorSelectionCandidate {
            runtime: runtime(),
            runtime_instance: id(90, TaskRuntimeInstanceId::from_normalized_identity),
            preservation: vec![ExecutorPreservationEvidence::new(
                ExecutorPreservationAxis::HostThread,
                id(91, ExecutorPreservationEvidenceId::from_normalized_identity),
            )],
        },
    )
    .expect_err("host-thread evidence cannot satisfy a CPU-pinned activation");

    assert!(
        diagnostic
            .0
            .contains("selected executor does not establish CPU preservation")
    );
}
