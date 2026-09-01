use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    Aarch64SameViewCopyElisionError, Aarch64SameViewCopyElisionPolicy,
    Aarch64SameViewCopyInstructionDisposition, aarch64_same_view_copy_elision_identity,
};

use super::super::elide_same_view_copy_before_return::tests::fixture::{budget, compare_fixture};
use super::*;

#[test]
fn descriptor_names_adjacent_body_topology() {
    use crate::rules::peephole_matching::InstructionPairTopology;

    assert_eq!(
        pattern::AARCH64_SAME_VIEW_COPY_BEFORE_COMPARE_ZERO_V1.topology(),
        InstructionPairTopology::AdjacentBodyInstructionsV1
    );
}

#[test]
fn independently_validates_non_terminal_same_view_copy_elision() {
    let fixture = compare_fixture();
    let plan = compute::compute_from_inputs(fixture.inputs(), budget()).unwrap();
    assert_eq!(
        plan.policy,
        Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1
    );
    assert_eq!(plan.actions.len(), 1);
    assert_eq!(plan.actions[0].copy.0, 1);
    assert_eq!(plan.actions[0].consumer.0, 2);
    assert_eq!(
        plan.actions[0].source.view,
        plan.actions[0].destination.view
    );
    assert_eq!(plan.actions[0].consumed.virtual_register.0, 2);
    assert!(matches!(
        plan.functions[0].blocks[0].instructions[0].disposition,
        Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 { consumer }
            if consumer.0 == 2
    ));

    let validated = validate::validate_from_inputs(fixture.inputs(), plan.clone()).unwrap();
    assert_eq!(validated.plan(), &plan);
    let decoded = crate::Aarch64SameViewCopyElisionPlan::decode(&plan.encode()).unwrap();
    assert_eq!(decoded, plan);
}

#[test]
fn independent_replay_rejects_authenticated_action_corruption() {
    let fixture = compare_fixture();
    let mut plan = compute::compute_from_inputs(fixture.inputs(), budget()).unwrap();
    plan.actions[0].consumed.virtual_register.0 += 1;
    plan.identity = aarch64_same_view_copy_elision_identity(&plan);
    assert_eq!(
        validate::validate_from_inputs(fixture.inputs(), plan),
        Err(Aarch64SameViewCopyElisionError::ArtifactMismatch)
    );
}

#[test]
fn rule_evaluation_budget_fails_closed() {
    let fixture = compare_fixture();
    let budget = OptimizationWorkBudget::new(1, 1, 1, 1, 2).unwrap();
    assert_eq!(
        compute::compute_from_inputs(fixture.inputs(), budget),
        Err(Aarch64SameViewCopyElisionError::BudgetExceeded(
            crate::Aarch64SameViewCopyElisionWorkAxis::RuleEvaluations
        ))
    );
}
