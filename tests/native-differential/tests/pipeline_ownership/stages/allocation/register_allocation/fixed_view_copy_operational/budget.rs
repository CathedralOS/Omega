//! Exact shared-entry usage and every budget-domain boundary.

use crate::tests::{
    FixedViewCopyError, OptimizationWorkBudget, OptimizationWorkUsage,
    OptimizedFixedViewCopyCustodyError,
};

use super::fixture::*;

const EXACT_USAGE: OptimizationWorkUsage = OptimizationWorkUsage {
    rule_evaluations: 3,
    candidates: 0,
    validation_steps: 32,
    commits: 0,
    iterations: 17,
};

fn exact_budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(3, 1, 32, 1, 17).unwrap()
}

fn fields(budget: OptimizationWorkBudget) -> [u64; 5] {
    [
        budget.rule_evaluations(),
        budget.candidates(),
        budget.validation_steps(),
        budget.commits(),
        budget.iterations(),
    ]
}

#[test]
fn shared_entry_fixed_view_copy_pins_exact_work_and_every_budget_domain_boundary() {
    for target in targets() {
        let exact = exact_budget();
        let staged = run(target, exact).unwrap();
        assert_eq!(staged.copies().plan().usage, EXACT_USAGE);
        assert_eq!(staged.copies().plan().budget, exact);
        assert_eq!(staged.custody().usage(), EXACT_USAGE);
        assert_eq!(staged.custody().copy_count(), 0);
        assert!(staged.copies().plan().copies.is_empty());

        for axis in 0..5 {
            // Zero-work axes have no representable first-under budget.
            if matches!(axis, 1 | 3) {
                continue;
            }
            let mut insufficient = fields(exact);
            insufficient[axis] -= 1;
            let Ok(insufficient) = OptimizationWorkBudget::new(
                insufficient[0],
                insufficient[1],
                insufficient[2],
                insufficient[3],
                insufficient[4],
            ) else {
                assert_eq!(fields(exact)[axis], 1);
                continue;
            };

            let first = run(target, insufficient).unwrap_err();
            let repeated = run(target, insufficient).unwrap_err();
            assert_eq!(first, repeated);
            assert_eq!(
                first,
                OptimizedFixedViewCopyCustodyError::Materialization(
                    FixedViewCopyError::BudgetExceeded {
                        required: EXACT_USAGE,
                        budget: insufficient,
                    },
                )
            );
        }
    }
}
