use crate::tests::*;
use optimization_core::OptimizationWorkBudget;

use super::fixture::{call_homes, exact_budget, stage, wide_budget};

#[test]
fn exact_budget_and_every_single_axis_first_under_fail_closed() {
    let source = call_homes(NativeTarget::linux_x64());
    let usage = stage(&source, wide_budget()).unwrap().receipt().usage();
    let exact = exact_budget(usage);
    assert_eq!(stage(&source, exact).unwrap().receipt().usage(), usage);

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
            stage(&source, budget),
            Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
                required: usage,
                budget,
            })
        );
    }
}
