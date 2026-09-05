use crate::tests::*;
use optimization_core::OptimizationWorkBudget;

use super::{
    super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle},
    fixture::{EXACT_USAGE, build, exact_budget, lower, validate},
};

#[test]
fn exact_budget_all_five_first_under_axes_and_cross_target_custody_fail_closed() {
    let insufficient = [
        OptimizationWorkBudget::new(6, 9, 15, 6, 10).unwrap(),
        OptimizationWorkBudget::new(7, 8, 15, 6, 10).unwrap(),
        OptimizationWorkBudget::new(7, 9, 14, 6, 10).unwrap(),
        OptimizationWorkBudget::new(7, 9, 15, 5, 10).unwrap(),
        OptimizationWorkBudget::new(7, 9, 15, 6, 9).unwrap(),
    ];
    for constructor in [
        reload_bundle as fn(NativeTarget) -> Bundle,
        original_bundle as fn(NativeTarget) -> Bundle,
    ] {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let source = build(constructor, target);
            assert!(lower(&source, exact_budget()).is_ok());
            for actual in insufficient {
                assert_eq!(
                    lower(&source, actual),
                    Err(
                        selected_instructions_to_register_homes::AbstractSpillMemoryEffectError::BudgetExceeded {
                            required: EXACT_USAGE,
                            budget: actual,
                        }
                    ),
                );
            }
        }
        let x86 = build(constructor, NativeTarget::linux_x64());
        let foreign = lower(&x86, exact_budget()).unwrap().plan().clone();
        let arm = build(constructor, NativeTarget::linux_arm64());
        assert_eq!(
            validate(&arm, foreign),
            Err(selected_instructions_to_register_homes::AbstractSpillMemoryEffectError::RootMismatch),
        );
    }
}
