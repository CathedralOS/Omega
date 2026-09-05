use crate::tests::*;

use super::fixture::{EXACT_USAGE, assign, source};

#[test]
fn every_first_under_budget_axis_fails() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let fixture = source(target);
        let required = EXACT_USAGE;
        let exact = OptimizationWorkBudget::new(
            required.rule_evaluations,
            required.candidates,
            required.validation_steps,
            required.commits,
            required.iterations,
        )
        .unwrap();
        assert_eq!(assign(&fixture, exact).unwrap().receipt().usage(), required);
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
                assign(&fixture, budget),
                Err(omega_selected_instructions_to_register_homes::FixedPrecoloredSegmentHomeError::BudgetExceeded {
                    required: actual,
                    budget: rejected,
                }) if actual == required && rejected == budget
            ));
        }
    }
}
