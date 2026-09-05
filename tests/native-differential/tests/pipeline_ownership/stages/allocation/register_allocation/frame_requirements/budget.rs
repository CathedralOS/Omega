use crate::tests::*;
use omega_optimization_core::OptimizationWorkBudget;

use super::fixture::{EXACT_USAGE, exact_budget, spill_source, stage};

#[test]
fn exact_budget_and_every_single_axis_first_under_fail_closed() {
    let target = NativeTarget::linux_x64();
    let source = spill_source(target);
    let environment = baseline_target_register_environment(target).unwrap();
    assert!(stage(&source, &environment, exact_budget()).is_ok());
    for budget in [
        OptimizationWorkBudget::new(1, 6, 7, 2, 7).unwrap(),
        OptimizationWorkBudget::new(2, 5, 7, 2, 7).unwrap(),
        OptimizationWorkBudget::new(2, 6, 6, 2, 7).unwrap(),
        OptimizationWorkBudget::new(2, 6, 7, 1, 7).unwrap(),
        OptimizationWorkBudget::new(2, 6, 7, 2, 6).unwrap(),
    ] {
        assert_eq!(
            stage(&source, &environment, budget),
            Err(SpillFrameRequirementError::BudgetExceeded {
                required: EXACT_USAGE,
                budget,
            }),
        );
    }
}
