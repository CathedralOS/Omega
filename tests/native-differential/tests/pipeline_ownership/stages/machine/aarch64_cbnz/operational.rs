//! Exact work accounting and independently replayed action custody.

use crate::tests::*;
use omega_post_allocation_machine_to_optimized_machine::{
    Aarch64CbnzFusionAction, Aarch64CbnzFusionError, Aarch64CbnzFusionWorkAxis,
    aarch64_cbnz_fusion_identity, optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz,
    validate_aarch64_cbnz_fusion,
};

fn run(
    fixture: &super::fixture::OperationalFixture,
    budget: OptimizationWorkBudget,
) -> Result<
    omega_post_allocation_machine_to_optimized_machine::ValidatedAarch64CbnzFusion,
    Aarch64CbnzFusionError,
> {
    let ranges = fixture.homes.legality_stage().live_range_stage();
    let selected = ranges.liveness_stage().selected_stage();
    optimize_aarch64_compare_i64_zero_branch_nonzero_to_cbnz(
        selected.selected(),
        ranges.liveness_stage().liveness(),
        fixture.machine.machine(),
        selected.register_environment().physical(),
        budget,
    )
}

fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap()
}

#[test]
fn two_fusions_pin_the_exact_nonzero_work_vector() {
    let fixture = super::fixture::operational_fixture();
    let fusion = run(&fixture, exact_budget()).unwrap();

    assert_eq!(fusion.receipt().action_count(), 2);
    assert_eq!(fusion.plan().usage.rule_evaluations, 5);
    assert_eq!(fusion.plan().usage.candidates, 2);
    assert_eq!(fusion.plan().usage.validation_steps, 2);
    assert_eq!(fusion.plan().usage.commits, 2);
    assert_eq!(fusion.plan().usage.iterations, 3);
}

#[test]
fn every_work_axis_rejects_the_first_unit_past_its_boundary() {
    let fixture = super::fixture::operational_fixture();
    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(4, 2, 2, 2, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(5, 1, 2, 2, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 1, 2, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 1, 3).unwrap(),
            Aarch64CbnzFusionWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 2, 2).unwrap(),
            Aarch64CbnzFusionWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            run(&fixture, budget),
            Err(Aarch64CbnzFusionError::BudgetExceeded(axis)),
            "the first unit beyond the exact {axis:?} budget must fail closed",
        );
    }
}

#[test]
fn independent_replay_rejects_reauthenticated_action_corruption() {
    let fixture = super::fixture::operational_fixture();
    let fusion = run(&fixture, exact_budget()).unwrap();
    let ranges = fixture.homes.legality_stage().live_range_stage();
    let selected = ranges.liveness_stage().selected_stage();
    let physical = selected.register_environment().physical();
    let corruptions: [fn(&mut Aarch64CbnzFusionAction); 5] = [
        |action| action.iteration += 1,
        |action| action.source_read.operand += 1,
        |action| action.source_read.units.clear(),
        |action| action.nzcv_units.clear(),
        |action| action.when_nonzero_edge = action.when_zero_edge,
    ];
    for corrupt in corruptions {
        let mut corrupted = fusion.plan().clone();
        corrupt(&mut corrupted.actions[0]);
        corrupted.identity = aarch64_cbnz_fusion_identity(&corrupted);

        assert_eq!(
            validate_aarch64_cbnz_fusion(
                selected.selected(),
                ranges.liveness_stage().liveness(),
                fixture.machine.machine(),
                physical,
                corrupted,
            ),
            Err(Aarch64CbnzFusionError::ArtifactMismatch),
            "independent replay must reject action corruption after identity reauthentication",
        );
    }
}
