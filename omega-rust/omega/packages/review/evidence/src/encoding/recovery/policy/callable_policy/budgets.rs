use super::tests::{fixture, recover};
use super::*;

#[test]
fn all_policy_recovery_budgets_apply_to_nested_callable_meaning() {
    let bytes = fixture().canonical_bytes().unwrap();
    let default = PackagePolicyRecoveryLimits::default();
    for (limits, error) in [
        (
            PackagePolicyRecoveryLimits {
                maximum_bytes: bytes.len() - 1,
                ..default
            },
            Error::InputTooLarge,
        ),
        (
            PackagePolicyRecoveryLimits {
                maximum_field_bytes: 0,
                ..default
            },
            Error::FieldTooLarge,
        ),
        (
            PackagePolicyRecoveryLimits {
                maximum_sequence_elements: 0,
                ..default
            },
            Error::ElementLimitExceeded,
        ),
        (
            PackagePolicyRecoveryLimits {
                maximum_owned_bytes: 0,
                ..default
            },
            Error::AllocationLimitExceeded,
        ),
        (
            PackagePolicyRecoveryLimits {
                maximum_depth: 0,
                ..default
            },
            Error::NestingLimitExceeded,
        ),
    ] {
        assert_eq!(
            PackagePolicyCallables::recover_canonical(&bytes, limits),
            Err(error)
        );
    }
    assert_eq!(recover(&bytes).unwrap(), fixture());
}

#[test]
fn shared_structural_requirement_depth_is_bounded_for_encoding_and_recovery() {
    let mut policy = fixture();
    let mut expression = PackageReviewBooleanExpression::Constant(true);
    for _ in 0..12 {
        expression = PackageReviewBooleanExpression::Not(Box::new(expression));
    }
    policy.callables[0]
        .checked_crash
        .structural_runtime_requirements = Some(vec![expression]);
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(
        PackagePolicyCallables::recover_canonical(
            &bytes,
            PackagePolicyRecoveryLimits {
                maximum_depth: 8,
                ..PackagePolicyRecoveryLimits::default()
            }
        ),
        Err(Error::NestingLimitExceeded)
    );
    assert_eq!(recover(&bytes).unwrap(), policy);
    let mut expression = PackageReviewBooleanExpression::Constant(true);
    for _ in 0..129 {
        expression = PackageReviewBooleanExpression::Not(Box::new(expression));
    }
    policy.callables[0]
        .checked_crash
        .structural_runtime_requirements = Some(vec![expression]);
    assert!(policy.canonical_bytes().is_err());
}
