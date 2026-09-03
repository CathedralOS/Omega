//! Work-budget accounting across every bounded axis.

use super::*;

#[test]
fn usage_checks_every_budget_axis() {
    let budget = OptimizationWorkBudget::new(1, 2, 3, 4, 5).unwrap();
    assert!(OptimizationWorkUsage::default().within(budget));
    assert!(
        !OptimizationWorkUsage {
            rule_evaluations: 2,
            ..OptimizationWorkUsage::default()
        }
        .within(budget)
    );
}
