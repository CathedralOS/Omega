use omega_optimization_core::OptimizationWorkBudget;
use omega_selected_instructions::VirtualRegisterId;

use super::super::{
    Aarch64SameViewCopyElisionAttemptOutcome, Aarch64SameViewCopyElisionError,
    Aarch64SameViewCopyElisionWorkAxis,
};

fn compute(
    fixture: &super::fixture::Fixture,
    budget: OptimizationWorkBudget,
) -> Result<super::super::Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionError> {
    super::super::compute::compute_from_inputs(fixture.inputs(), budget)
}

#[test]
fn exact_pair_elides_once_and_independent_replay_agrees() {
    let fixture = super::fixture::fixture();
    let plan = compute(&fixture, super::fixture::budget()).unwrap();
    let validated =
        super::super::validate::validate_from_inputs(fixture.inputs(), plan.clone()).unwrap();

    assert_eq!(validated.plan(), &plan);
    assert_eq!(validated.receipt().action_count(), 1);
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
    assert_eq!(
        plan.actions[0].source.view,
        plan.actions[0].destination.view
    );
    assert_eq!(
        plan.actions[0].destination.virtual_register,
        plan.actions[0].consumed.virtual_register
    );

    let repeated = compute(&fixture, super::fixture::budget()).unwrap();
    assert_eq!(
        repeated, plan,
        "immutable-source reconstruction is deterministic"
    );
}

fn two_pair_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap()
}

#[test]
fn two_pairs_pin_the_exact_nonzero_work_vector() {
    let fixture = super::fixture::two_pair_fixture();
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
    let fixture = super::fixture::two_pair_fixture();
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
    let mut different_storage = super::fixture::fixture();
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
    assert!(plan.actions.is_empty());
    assert_eq!(
        plan.attempts[0].outcome,
        Aarch64SameViewCopyElisionAttemptOutcome::DifferentPhysicalStorage
    );

    let mut wrong_value = super::fixture::fixture();
    let returned = match &mut wrong_value.selected.functions[0].blocks[0].terminator {
        omega_selected_instructions::SelectedTerminator::Return { instruction, .. } => instruction,
        _ => unreachable!(),
    };
    returned.operands[0].virtual_register = VirtualRegisterId(3);
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

    let mut semantic = super::fixture::fixture();
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
