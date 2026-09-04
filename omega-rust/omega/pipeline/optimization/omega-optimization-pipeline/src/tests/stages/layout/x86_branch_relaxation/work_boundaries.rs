//! Exact work-accounting success and first-over-boundary refusal.

use crate::tests::*;

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
