use super::super::{Aarch64SameViewCopyElisionError, aarch64_same_view_copy_elision_identity};

#[test]
fn independent_replay_rejects_reauthenticated_action_corruption() {
    let fixture = super::fixture::fixture();
    let plan =
        super::super::compute::compute_from_inputs(fixture.inputs(), super::fixture::budget())
            .unwrap();
    let corruptions: [fn(&mut super::super::Aarch64SameViewCopyElisionPlan); 5] = [
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
