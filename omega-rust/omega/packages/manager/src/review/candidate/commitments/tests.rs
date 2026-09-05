//! Boundary test for the canonical build-observation identity consumed here.

use super::build_observation_commitment;

#[test]
fn review_delegates_build_observation_identity_to_its_owner() {
    let summary = build_evaluation::test_support::replayable_unknown_descriptor_summary();

    assert_eq!(
        build_observation_commitment(&summary),
        summary.identity().digest()
    );
}
