use super::{candidate, id, runtime};
use crate::{
    ActivationCarryObligations, ExecutorPreservationAxis, ExecutorPreservationEvidence,
    ExecutorPreservationEvidenceId, ExecutorSelectionCandidate, TaskRuntimeInstanceId,
    validate_activation_plan, validate_executor_selection,
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

#[test]
fn executor_selection_requires_only_the_activation_affinity_axes() {
    let mut portable = candidate();
    // A portable activation has no CPU-pinned live place at its crossing;
    // relaxing the row's demand is what makes the cleared bit honest.
    portable.canonical_suspension_crossings[0].live_carry[0]
        .effective
        .cpu = language_core::CarryCpu::Any;
    portable.canonical_suspension_crossings[0].preserve_cpu = false;
    portable.carry_obligations = ActivationCarryObligations::none();
    let plan = validate_activation_plan(portable).expect("portable activation plan");
    let instance = id(92, TaskRuntimeInstanceId::from_normalized_identity);
    let selection = validate_executor_selection(
        &plan,
        ExecutorSelectionCandidate {
            runtime: runtime(),
            runtime_instance: instance,
            preservation: Vec::new(),
        },
    )
    .expect("portable activation needs no affinity evidence");

    assert_eq!(selection.candidate().runtime_instance, instance);
    assert_eq!(selection.plan(), &plan);
    assert_ne!(selection.identity().normalized_identity(), 0);
}
