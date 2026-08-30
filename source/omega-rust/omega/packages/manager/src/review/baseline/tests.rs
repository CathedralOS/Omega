use super::encoding::{
    Decoder, Encoder, decode_replay_record_option, decode_resolution, encode_replay_record_option,
    encode_resolution, replay_parent_binding,
};
use super::validation::replay_record_limits;
use super::*;
use omega_build_evaluation::{
    BuildObservationClass, capture_verified_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

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
fn replay_record_option_framing_round_trips_compiler_bytes() {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-baseline-replay-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create replay framing fixture");
    std::fs::write(
        project.join("build.omg"),
        r#"use omega::language::std::filesystem_host;

target windows_x64 { }

machine build(builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{
    builder.application("review-baseline-replay");
    let source: &[u8] in Path = builder.source.resolve("main.omg");
    let bytes: [u8; 144];
    let status: i32 = builder.filesystem.read_metadata(source, &mut bytes);
}

"#,
    )
    .expect("write replay framing build");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n")
        .expect("write replay framing source");
    let compilation =
        omega_compiler::compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .expect("compile replay framing fixture");
    let summary = compilation
        .build_observation_summary()
        .expect("filesystem build publishes observations");
    assert!(summary.source_inputs_replay_verified());
    let limits = ReviewOnlyBaselineLimits::default();
    let replay =
        capture_verified_build_filesystem_replay_record(summary, replay_record_limits(limits))
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
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn baseline_retains_unknown_descriptor_close_replay_custody() {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-baseline-unknown-close-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create unknown-close baseline fixture");
    std::fs::write(
        project.join("build.omg"),
        r#"use omega::language::std::filesystem_host;

target windows_x64 { }

machine build(builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{
    builder.application("review-baseline-unknown-close");
    let status: i32 = builder.filesystem.close(-1);
}
"#,
    )
    .expect("write unknown-close baseline build");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n")
        .expect("write unknown-close baseline source");
    let compilation =
        omega_compiler::compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .expect("compile unknown-close baseline fixture");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown close publishes build observations");
    assert!(summary.operation_replay_verified());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);

    let limits = ReviewOnlyBaselineLimits::default();
    let replay =
        capture_verified_build_filesystem_replay_record(summary, replay_record_limits(limits))
            .expect("capture unknown-close replay record")
            .expect("verified unknown close retains replay custody");
    let mut encoder = Encoder::bounded(limits.maximum_capsule_bytes);
    encode_replay_record_option(&mut encoder, Some(&replay)).expect("frame unknown-close replay");
    let framed = encoder.finish().expect("finish unknown-close replay frame");
    let mut decoder = Decoder::new(&framed);
    let recovered = decode_replay_record_option(&mut decoder, limits)
        .expect("recover unknown-close replay frame")
        .expect("unknown-close replay frame is present");
    decoder
        .finish()
        .expect("unknown-close replay consumes frame");
    let rehydrated = rehydrate_review_only_build_filesystem_replay_record(
        &recovered,
        replay_record_limits(limits),
    )
    .expect("unknown-close baseline rehydrates through compiler replay custody");
    assert_eq!(
        rehydrated
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![8]
    );
    assert!(!rehydrated.has_output_attempts());

    let _ = std::fs::remove_dir_all(project);
}
