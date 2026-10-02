//! Work-budget accounting across every bounded axis.

use super::super::OptimizationWorkUsage;
use super::OptimizationWorkBudget;

#[test]
fn usage_checks_every_budget_axis() {
    let budget = OptimizationWorkBudget::new(1, 2, 3, 4, 5).unwrap();
    assert!(OptimizationWorkUsage::default().within(budget));
    let exact = OptimizationWorkUsage {
        rule_evaluations: 1,
        candidates: 2,
        validation_steps: 3,
        commits: 4,
        iterations: 5,
    };
    assert!(exact.within(budget));
    for excess in [
        OptimizationWorkUsage {
            rule_evaluations: 2,
            ..exact
        },
        OptimizationWorkUsage {
            candidates: 3,
            ..exact
        },
        OptimizationWorkUsage {
            validation_steps: 4,
            ..exact
        },
        OptimizationWorkUsage {
            commits: 5,
            ..exact
        },
        OptimizationWorkUsage {
            iterations: 6,
            ..exact
        },
    ] {
        assert!(
            !excess.within(budget),
            "over-budget work admitted: {excess:?}"
        );
    }
}
