//! Exact work-accounting success and first-over-boundary refusal.

use resolved_layout_to_resolved_layout::validate_optimized_x86_branch_relaxation;

use crate::tests::{
    OptimizationWorkBudget, OptimizedX86BranchRelaxationError, X86BranchRelaxationWorkAxis,
    selected_lowering_budget, stage_optimized_layout_independent_selected_form_encoding,
    stage_optimized_post_allocation_machine_plan, stage_optimized_resolved_selected_form_layout,
    stage_optimized_x86_branch_relaxation,
};
#[test]
fn exact_usage_and_every_one_below_budget_are_typed() {
    let exact =
        super::fixture::stage_with_budget(OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap())
            .unwrap();
    assert_eq!(exact.usage().rule_evaluations, 5);
    assert_eq!(exact.usage().candidates, 2);
    assert_eq!(exact.usage().validation_steps, 2);
    assert_eq!(exact.usage().commits, 2);
    assert_eq!(exact.usage().iterations, 3);

    for (budget, axis) in [
        (
            OptimizationWorkBudget::new(4, 2, 2, 2, 3).unwrap(),
            X86BranchRelaxationWorkAxis::RuleEvaluations,
        ),
        (
            OptimizationWorkBudget::new(5, 1, 2, 2, 3).unwrap(),
            X86BranchRelaxationWorkAxis::Candidates,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 1, 2, 3).unwrap(),
            X86BranchRelaxationWorkAxis::ValidationSteps,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 1, 3).unwrap(),
            X86BranchRelaxationWorkAxis::Commits,
        ),
        (
            OptimizationWorkBudget::new(5, 2, 2, 2, 2).unwrap(),
            X86BranchRelaxationWorkAxis::Iterations,
        ),
    ] {
        assert_eq!(
            super::fixture::stage_with_budget(budget),
            Err(OptimizedX86BranchRelaxationError::BudgetExceeded(axis)),
            "a one-below {axis:?} budget must fail on its typed axis",
        );
    }
}

#[test]
fn the_measured_budget_also_admits_through_independent_replay() {
    let homes = super::fixture::physical_homes();
    let machine = stage_optimized_post_allocation_machine_plan(&homes).unwrap();
    let selected_stage = homes
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected();
    let physical = selected_stage.register_environment().physical();
    let encoding = stage_optimized_layout_independent_selected_form_encoding(
        selected, &machine, physical, None,
    )
    .unwrap();
    let baseline =
        stage_optimized_resolved_selected_form_layout(selected, &machine, physical, &encoding)
            .unwrap();
    let exact = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline,
        OptimizationWorkBudget::new(5, 2, 2, 2, 3).unwrap(),
    )
    .unwrap();
    // The validator re-runs the same bounded fixed point, so the exact budget
    // admits through independent replay as well as through production.
    assert_eq!(
        validate_optimized_x86_branch_relaxation(
            selected, &machine, physical, &encoding, &baseline, &exact,
        ),
        Ok(()),
    );

    // A recorded budget forged below the measured usage fails closed before
    // replay even when every other field is authentic.
    let mut forged = exact.clone();
    forged.corrupt_recorded_budget_and_reauthenticate_for_test();
    assert_eq!(
        validate_optimized_x86_branch_relaxation(
            selected, &machine, physical, &encoding, &baseline, &forged,
        ),
        Err(OptimizedX86BranchRelaxationError::ArtifactMismatch),
    );

    // Production under an over-generous budget still publishes the same
    // measured usage; the budget ceiling is not part of the evidence roster.
    let generous = stage_optimized_x86_branch_relaxation(
        selected,
        &machine,
        physical,
        &encoding,
        &baseline,
        selected_lowering_budget(),
    )
    .unwrap();
    assert_eq!(generous.usage(), exact.usage());
    assert_ne!(generous.budget(), exact.budget());
    assert_eq!(
        validate_optimized_x86_branch_relaxation(
            selected, &machine, physical, &encoding, &baseline, &generous,
        ),
        Ok(()),
    );
}
