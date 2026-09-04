//! Authenticated one-field corruption coverage for generic machine custody.

#[test]
fn every_post_allocation_custody_field_rejects_after_outer_reauthentication() {
    super::super::post_allocation_custody_corruption::assert_every_field_rejects(
        super::fixture::staged_realization(),
    );
}
