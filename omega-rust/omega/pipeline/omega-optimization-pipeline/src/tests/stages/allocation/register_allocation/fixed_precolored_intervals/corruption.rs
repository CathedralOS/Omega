use crate::tests::*;

use super::fixture::{analyze, exact_budget, source, validate};

#[test]
fn roots_rows_usage_and_cross_target_custody_fail_closed() {
    let x86 = source(NativeTarget::linux_x64());
    let valid = analyze(&x86, exact_budget()).unwrap();
    let identity = valid.receipt().identity();

    let mut corrupted = valid.plan().clone();
    corrupted.ranges =
        omega_selected_instructions_to_register_homes::LiveRangeIdentity::from_bytes([0x31; 32]);
    assert_ne!(
        omega_selected_instructions_to_register_homes::fixed_precolored_interval_plan_identity(
            &corrupted
        ),
        identity,
    );
    assert!(matches!(
        validate(&x86, corrupted),
        Err(omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError::RootMismatch)
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].intervals[0].end.0 += 1;
    assert_ne!(
        omega_selected_instructions_to_register_homes::fixed_precolored_interval_plan_identity(
            &corrupted
        ),
        identity,
    );
    assert!(matches!(
        validate(&x86, corrupted),
        Err(omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError::NonCanonicalFunctions)
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.functions[0].intervals[0].view.0 += 1;
    assert!(matches!(
        validate(&x86, corrupted),
        Err(omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError::NonCanonicalFunctions)
    ));

    let mut corrupted = valid.plan().clone();
    corrupted.usage.validation_steps += 1;
    assert!(matches!(
        validate(&x86, corrupted),
        Err(omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError::UsageMismatch)
    ));

    let arm = source(NativeTarget::linux_arm64());
    assert!(matches!(
        validate(&arm, valid.plan().clone()),
        Err(omega_selected_instructions_to_register_homes::FixedPrecoloredIntervalError::RootMismatch)
    ));
}
