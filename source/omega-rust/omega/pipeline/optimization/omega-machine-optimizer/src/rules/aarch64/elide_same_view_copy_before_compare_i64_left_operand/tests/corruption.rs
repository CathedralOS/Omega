use crate::{Aarch64SameViewCopyElisionError, aarch64_same_view_copy_elision_identity};

use super::super::super::elide_same_view_copy_before_return::tests::fixture;

#[test]
fn independent_replay_rejects_every_reauthenticated_action_field_corruption() {
    let fixture = fixture::compare_i64_left_operand_fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), fixture::budget()).unwrap();
    let corruptions: [fn(&mut crate::Aarch64SameViewCopyElisionPlan); 5] = [
        |plan| plan.actions[0].iteration += 1,
        |plan| plan.actions[0].source.operand += 1,
        |plan| plan.actions[0].destination.storage_units.clear(),
        |plan| plan.actions[0].consumed.virtual_register.0 += 1,
        |plan| plan.actions[0].consumer.0 += 1,
    ];

    for corrupt in corruptions {
        let mut corrupted = plan.clone();
        corrupt(&mut corrupted);
        corrupted.identity = aarch64_same_view_copy_elision_identity(&corrupted);
        assert_eq!(
            super::super::validate::validate_from_inputs(fixture.inputs(), corrupted),
            Err(Aarch64SameViewCopyElisionError::ArtifactMismatch)
        );
    }
}

#[test]
fn exact_policy_substitution_changes_identity_and_is_rejected() {
    let fixture = fixture::compare_i64_left_operand_fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), fixture::budget()).unwrap();
    let original_identity = plan.identity;
    let mut substituted = plan;
    substituted.policy =
        crate::Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1;
    substituted.identity = aarch64_same_view_copy_elision_identity(&substituted);
    assert_ne!(substituted.identity, original_identity);
    assert_eq!(
        super::super::validate::validate_from_inputs(fixture.inputs(), substituted),
        Err(Aarch64SameViewCopyElisionError::ArtifactMismatch)
    );
}
