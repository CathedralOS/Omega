use omega_optimization_core::OptimizationWorkBudget;
use omega_selected_instructions::VirtualRegisterId;

use crate::{
    Aarch64SameViewCopyElisionAttemptOutcome, Aarch64SameViewCopyElisionError,
    Aarch64SameViewCopyElisionPolicy, Aarch64SameViewCopyElisionWorkAxis,
    Aarch64SameViewCopyInstructionDisposition,
};

use super::super::super::same_view_copy_elision::test_support::fixture;

fn compute(
    fixture: &fixture::Fixture,
    budget: OptimizationWorkBudget,
) -> Result<crate::Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    super::super::compute::compute_from_inputs(fixture.inputs(), budget)
}

#[test]
fn exact_pair_elides_once_and_independent_replay_agrees() {
    let fixture = fixture::compare_i64_left_operand_fixture();
    let plan = compute(&fixture, fixture::budget()).unwrap();
    let validated =
        super::super::validate::validate_from_inputs(fixture.inputs(), plan.clone()).unwrap();

    assert_eq!(validated.plan(), &plan);
    assert_eq!(validated.receipt().action_count(), 1);
    assert_eq!(
        plan.policy,
        Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareI64LeftOperandV1
    );
    assert_eq!(plan.usage.rule_evaluations, 2);
    assert_eq!(plan.usage.candidates, 1);
    assert_eq!(plan.usage.validation_steps, 1);
    assert_eq!(plan.usage.commits, 1);
    assert_eq!(plan.usage.iterations, 2);
    assert_eq!(
        plan.attempts
            .iter()
            .map(|attempt| attempt.outcome)
            .collect::<Vec<_>>(),
        [
            Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision,
            Aarch64SameViewCopyElisionAttemptOutcome::AlreadyElided,
        ]
    );
    assert!(matches!(
        plan.functions[0].blocks[0].instructions[0].disposition,
        Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 { consumer }
            if consumer.0 == 2
    ));
    assert_eq!(compute(&fixture, fixture::budget()).unwrap(), plan);
}

fn two_pair_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap()
}

#[test]
fn two_pairs_pin_the_exact_nonzero_work_vector() {
    let fixture = fixture::two_pair_compare_i64_left_operand_fixture();
    let plan = compute(&fixture, two_pair_budget()).unwrap();
    let validated =
        super::super::validate::validate_from_inputs(fixture.inputs(), plan.clone()).unwrap();

    assert_eq!(validated.plan(), &plan);
    assert_eq!(validated.receipt().action_count(), 2);
    assert_eq!(plan.actions.len(), 2);
    assert_eq!(plan.usage.rule_evaluations, 5);
    assert_eq!(plan.usage.candidates, 2);
    assert_eq!(plan.usage.validation_steps, 2);
    assert_eq!(plan.usage.commits, 2);
    assert_eq!(plan.usage.iterations, 3);
}

#[test]
fn every_work_axis_rejects_the_first_unit_past_its_boundary() {
    let fixture = fixture::two_pair_compare_i64_left_operand_fixture();
    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(4, 2, 2, 2, 3).unwrap(),
            Aarch64SameViewCopyElisionWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(5, 1, 2, 2, 3).unwrap(),
            Aarch64SameViewCopyElisionWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 1, 2, 3).unwrap(),
            Aarch64SameViewCopyElisionWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 1, 3).unwrap(),
            Aarch64SameViewCopyElisionWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 2, 2).unwrap(),
            Aarch64SameViewCopyElisionWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            compute(&fixture, budget),
            Err(Aarch64SameViewCopyElisionError::BudgetExceeded(axis)),
            "the first unit beyond the exact {axis:?} budget must fail closed",
        );
    }
}

#[test]
fn valid_non_candidates_are_retained_with_typed_outcomes() {
    let mut different_storage = fixture::compare_i64_left_operand_fixture();
    let x1 = different_storage
        .physical
        .model()
        .view_named("x1")
        .unwrap()
        .clone();
    let source = &mut different_storage.source.functions[0].blocks[0].instructions[0];
    source.operands[0].view = x1.id;
    source.operands[0].class = x1.class;
    source.operands[0].storage_units = x1.units.clone();
    source.operands[0].read_units = x1.units.clone();
    source.unit_uses = x1.units;
    let plan = compute(
        &different_storage,
        OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        plan.attempts[0].outcome,
        Aarch64SameViewCopyElisionAttemptOutcome::DifferentPhysicalStorage
    );

    let mut wrong_value = fixture::compare_i64_left_operand_fixture();
    wrong_value.selected.functions[0].blocks[0].instructions[1].operands[0].virtual_register =
        VirtualRegisterId(3);
    wrong_value.source.functions[0].blocks[0].instructions[1].operands[0].virtual_register =
        VirtualRegisterId(3);
    let plan = compute(
        &wrong_value,
        OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        plan.attempts[0].outcome,
        Aarch64SameViewCopyElisionAttemptOutcome::DestinationNotConsumed
    );

    let mut semantic = fixture::compare_i64_left_operand_fixture();
    semantic.selected.functions[0].blocks[0].instructions[0]
        .provenance
        .values
        .clear();
    let plan = compute(
        &semantic,
        OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        plan.attempts[0].outcome,
        Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance
    );
}

#[test]
fn copy_consumed_only_by_the_right_operand_is_outside_this_exact_rule() {
    let mut fixture = fixture::compare_i64_left_operand_fixture();
    let selected_compare = &mut fixture.selected.functions[0].blocks[0].instructions[1];
    selected_compare.operands.swap(0, 1);
    selected_compare.operands[0].operand = 0;
    selected_compare.operands[1].operand = 1;

    let machine_compare = &mut fixture.source.functions[0].blocks[0].instructions[1];
    machine_compare.operands.swap(0, 1);
    machine_compare.operands[0].operand = 0;
    machine_compare.operands[1].operand = 1;

    let plan = compute(
        &fixture,
        OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        plan.attempts[0].outcome,
        Aarch64SameViewCopyElisionAttemptOutcome::DestinationNotConsumed
    );
    assert!(plan.actions.is_empty());
}

#[test]
fn compare_left_origin_must_retain_the_copy_source_value() {
    let mut proposal_fixture = fixture::compare_i64_left_operand_fixture();
    let compared = proposal_fixture.selected.functions[0].blocks[0].instructions[1].operands[0]
        .virtual_register;
    let register = proposal_fixture.selected.functions[0]
        .virtual_registers
        .iter_mut()
        .find(|register| register.id == compared)
        .unwrap();
    let omega_selected_instructions::VirtualRegisterOrigin::InstructionResult {
        source_value, ..
    } = &mut register.origin
    else {
        panic!("fixture compare-left value is the copy result")
    };
    *source_value = psi_core::ValueId::new(9).unwrap();
    let proposed = compute(
        &proposal_fixture,
        OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
    )
    .unwrap();
    assert_eq!(
        proposed.attempts[0].outcome,
        Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance
    );

    let valid_fixture = fixture::compare_i64_left_operand_fixture();
    let valid = compute(&valid_fixture, fixture::budget()).unwrap();
    assert_eq!(
        super::super::validate::validate_from_inputs(proposal_fixture.inputs(), valid),
        Err(Aarch64SameViewCopyElisionError::ArtifactMismatch)
    );
}
