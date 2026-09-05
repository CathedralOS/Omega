use crate::tests::*;
use omega_optimization_core::OptimizationWorkBudget;

use super::{
    super::recursive_reload_value_homes::{Bundle, original_bundle, reload_bundle},
    fixture::{EXACT_USAGE, build, constrain, exact_budget, validate},
};

#[test]
fn exact_budget_all_five_first_under_axes_and_cross_target_custody_fail_closed() {
    let insufficient = [
        OptimizationWorkBudget::new(6, 15, 33, 18, 22).unwrap(),
        OptimizationWorkBudget::new(7, 14, 33, 18, 22).unwrap(),
        OptimizationWorkBudget::new(7, 15, 32, 18, 22).unwrap(),
        OptimizationWorkBudget::new(7, 15, 33, 17, 22).unwrap(),
        OptimizationWorkBudget::new(7, 15, 33, 18, 21).unwrap(),
    ];
    for constructor in [
        reload_bundle as fn(NativeTarget) -> Bundle,
        original_bundle as fn(NativeTarget) -> Bundle,
    ] {
        for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
            let source = build(constructor, target);
            assert!(constrain(&source, exact_budget()).is_ok());
            for actual in insufficient {
                assert_eq!(
                    constrain(&source, actual),
                    Err(
                        omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintError::BudgetExceeded {
                            required: EXACT_USAGE,
                            budget: actual,
                        }
                    ),
                );
            }
        }
        let x86 = build(constructor, NativeTarget::linux_x64());
        let foreign = constrain(&x86, exact_budget()).unwrap().plan().clone();
        let arm = build(constructor, NativeTarget::linux_arm64());
        assert_eq!(
            validate(&arm, foreign),
            Err(omega_selected_instructions_to_register_homes::AbstractSpillAccessConstraintError::RootMismatch),
        );
    }
}
