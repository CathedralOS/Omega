use crate::tests::*;

use super::fixture::{call_requirements, exact_budget, stage, wide_budget};

#[test]
fn exact_budget_and_every_single_axis_first_under_fail_closed() {
    let (requirements, environment) = call_requirements(NativeTarget::linux_x64());
    let usage = stage(&requirements, &environment, wide_budget())
        .unwrap()
        .receipt()
        .usage();
    assert_eq!(
        usage,
        OptimizationWorkUsage {
            rule_evaluations: 15,
            candidates: 18,
            validation_steps: 60,
            commits: 30,
            iterations: 57,
        }
    );
    assert_eq!(
        stage(&requirements, &environment, exact_budget(usage))
            .unwrap()
            .receipt()
            .usage(),
        usage
    );

    for budget in [
        OptimizationWorkBudget::new(
            usage.rule_evaluations - 1,
            usage.candidates,
            usage.validation_steps,
            usage.commits,
            usage.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates - 1,
            usage.validation_steps,
            usage.commits,
            usage.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps - 1,
            usage.commits,
            usage.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps,
            usage.commits - 1,
            usage.iterations,
        )
        .unwrap(),
        OptimizationWorkBudget::new(
            usage.rule_evaluations,
            usage.candidates,
            usage.validation_steps,
            usage.commits,
            usage.iterations - 1,
        )
        .unwrap(),
    ] {
        assert_eq!(
            stage(&requirements, &environment, budget),
            Err(NonAuthoritativeCalleeSaveStorageError::BudgetExceeded {
                required: usage,
                budget,
            })
        );
    }
}
