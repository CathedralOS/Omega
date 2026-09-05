use crate::tests::*;

use super::fixture::{X64_EXACT_USAGE, analyze, exact_budget, source};

#[test]
fn exact_budget_succeeds_and_every_first_under_axis_fails() {
    let fixture = source(NativeTarget::linux_x64());
    let required = X64_EXACT_USAGE;
    let exact = exact_budget(NativeTarget::linux_x64());
    assert_eq!(
        analyze(&fixture, exact).unwrap().receipt().usage(),
        required
    );

    let budgets = [
        OptimizationWorkBudget::new(
            required.rule_evaluations - 1,
            required.candidates,
            required.validation_steps,
            required.commits,
            required.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            required.rule_evaluations,
            required.candidates - 1,
            required.validation_steps,
            required.commits,
            required.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            required.rule_evaluations,
            required.candidates,
            required.validation_steps - 1,
            required.commits,
            required.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            required.rule_evaluations,
            required.candidates,
            required.validation_steps,
            required.commits - 1,
            required.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            required.rule_evaluations,
            required.candidates,
            required.validation_steps,
            required.commits,
            required.iterations - 1,
        )
        .unwrap(),
    ];
    for budget in budgets {
        assert!(matches!(
            analyze(&fixture, budget),
            Err(selected_instructions_to_register_homes::FixedPrecoloredSplitRequirementError::BudgetExceeded {
                required: actual,
                budget: rejected,
            }) if actual == required && rejected == budget
        ));
    }
}
