use super::encoding::{
    Decoder, Encoder, decode_replay_record_option, decode_resolution, encode_replay_record_option,
    encode_resolution, replay_parent_binding,
};
use super::validation::replay_record_limits;
use super::*;
use omega_build_evaluation::{
    capture_verified_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};

#[test]
fn baseline_git_resolution_rejects_content_not_derived_from_its_tree() {
    use omega_package_source::{GitCommitId, GitTreeId, ImmutableSourceResolution};

    let resolution = ImmutableSourceResolution::git(
        GitCommitId::parse_hex(&"01".repeat(20)).unwrap(),
        GitTreeId::parse_hex(&"02".repeat(20)).unwrap(),
    )
    .unwrap();
    let mut encoder = Encoder::bounded(256);
    encode_resolution(&mut encoder, &resolution).unwrap();
    let mut encoded = encoder.finish().unwrap();

    let mut decoder = Decoder::new(&encoded);
    assert_eq!(decode_resolution(&mut decoder).unwrap(), resolution);
    decoder.finish().unwrap();

    *encoded.last_mut().unwrap() ^= 1;
    assert!(decode_resolution(&mut Decoder::new(&encoded)).is_err());
}

#[test]
fn replay_record_option_framing_round_trips_owner_constructed_bytes() {
    let summary = omega_build_evaluation::test_support::replayable_unknown_descriptor_summary();
    let limits = ReviewOnlyBaselineLimits::default();
    let replay =
        capture_verified_build_filesystem_replay_record(&summary, replay_record_limits(limits))
            .expect("capture replay record")
            .expect("verified replay record");

    let mut encoder = Encoder::bounded(limits.maximum_capsule_bytes);
    encode_replay_record_option(&mut encoder, Some(&replay)).expect("frame replay option");
    let framed = encoder.finish().expect("finish replay option");
    let mut decoder = Decoder::new(&framed);
    let recovered = decode_replay_record_option(&mut decoder, limits)
        .expect("recover framed replay option")
        .expect("recovered replay option is present");
    decoder.finish().expect("replay option consumes its frame");
    assert_eq!(recovered, replay);
    rehydrate_review_only_build_filesystem_replay_record(&recovered, replay_record_limits(limits))
        .expect("recovered manager framing remains accepted by its semantic owner");

    let parent = [7; 32];
    assert_eq!(
        replay_parent_binding(parent, recovered.commitment()),
        replay_parent_binding(parent, replay.commitment())
    );
    assert_ne!(
        replay_parent_binding(parent, recovered.commitment()),
        replay_parent_binding([8; 32], recovered.commitment())
    );
    assert_eq!(
        decode_replay_record_option(&mut Decoder::new(&[0]), limits)
            .expect("absent replay option")
            .as_ref(),
        None
    );
    assert!(decode_replay_record_option(&mut Decoder::new(&[2]), limits).is_err());
}
