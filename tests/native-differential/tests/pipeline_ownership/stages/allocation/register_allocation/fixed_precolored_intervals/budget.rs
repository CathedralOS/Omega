use super::fixture::{EXACT_USAGE, analyze, source};
use crate::tests::{NativeTarget, OptimizationWorkBudget};

#[test]
fn every_representable_first_under_budget_fails_before_publication() {
    let source = source(NativeTarget::linux_x64());
    for budget in [
        OptimizationWorkBudget::new(1, 3, 10, 4, 2).unwrap(),
        OptimizationWorkBudget::new(1, 4, 9, 4, 2).unwrap(),
        OptimizationWorkBudget::new(1, 4, 10, 3, 2).unwrap(),
        OptimizationWorkBudget::new(1, 4, 10, 4, 1).unwrap(),
    ] {
        assert!(matches!(
            analyze(&source, budget),
            Err(selected_instructions_to_selected_instructions::FixedPrecoloredIntervalError::BudgetExceeded { required, budget: actual })
                if required == EXACT_USAGE && actual == budget
        ));
    }
}

#[test]
fn exact_budget_is_accepted() {
    let source = source(NativeTarget::linux_x64());
    let budget = OptimizationWorkBudget::new(1, 4, 10, 4, 2).unwrap();
    assert_eq!(
        analyze(&source, budget).unwrap().receipt().usage(),
        EXACT_USAGE
    );
}
