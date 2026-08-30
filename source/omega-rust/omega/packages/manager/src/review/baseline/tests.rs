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
    assert!(summary.filesystem_replay_verdict().replays_source_inputs());
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
fn baseline_retains_operand_free_unknown_descriptor_failure_replay_custody() {
    assert_baseline_retains_unknown_descriptor_failure(
        "unknown-sync-data",
        "let status: i32 = builder.filesystem.sync_data(-1);",
        44,
        &[],
    );
}

#[test]
fn baseline_retains_unknown_descriptor_seek_failure_replay_custody() {
    assert_baseline_retains_unknown_descriptor_failure(
        "unknown-seek",
        "let position: i64 = builder.filesystem.seek(-1, -17, 2);",
        10,
        &[
            psi_checked_interpreter::FilesystemScalarOperandValue::I64(-17),
            psi_checked_interpreter::FilesystemScalarOperandValue::I32(2),
        ],
    );
}

#[test]
fn baseline_retains_unknown_descriptor_write_operation_replay_custody() {
    assert_baseline_retains_unknown_descriptor_failure(
        "unknown-change-file-owner",
        "let status: i32 = builder.filesystem.change_file_owner(-1, -1, -2);",
        49,
        &[
            psi_checked_interpreter::FilesystemScalarOperandValue::I32(-1),
            psi_checked_interpreter::FilesystemScalarOperandValue::I32(-2),
        ],
    );
}

#[test]
fn baseline_retains_unknown_descriptor_set_file_times_replay_custody() {
    let replay = unknown_descriptor_failure_baseline(
        "unknown-set-file-times",
        r#"let mut times: [u8; 32];
    times[0] = 11;
    times[16] = 29;
    times[31] = 173;
    let status: i32 = builder.filesystem.set_file_times(-1, &mut times);"#,
    );
    let [attempt] = replay.attempts() else {
        panic!("set_file_times baseline retains one operation")
    };
    assert_eq!(attempt.operation_tag(), 42);
    let [resolution] = attempt.mutable_byte_operand_resolutions() else {
        panic!("set_file_times baseline retains one resolution-time carrier")
    };
    let [carrier] = attempt.mutable_byte_operands() else {
        panic!("set_file_times baseline retains one provider carrier")
    };
    assert_eq!(resolution.operand_ordinal(), 1);
    assert_eq!(resolution.bytes().len(), 32);
    assert_eq!(resolution.bytes()[0], 11);
    assert_eq!(resolution.bytes()[16], 29);
    assert_eq!(resolution.bytes()[31], 173);
    assert_eq!(carrier.operand_ordinal(), 1);
    assert_eq!(resolution.bytes(), carrier.pre_bytes());
    assert_eq!(carrier.pre_bytes(), carrier.post_bytes());
    assert!(!replay.has_output_attempts());
}

#[test]
fn baseline_retains_unknown_descriptor_read_replay_custody() {
    let replay = unknown_descriptor_failure_baseline(
        "unknown-read-at",
        r#"let mut buffer: [u8; 47];
    buffer[0] = 11;
    buffer[23] = 29;
    buffer[46] = 173;
    let count: i64 = builder.filesystem.read_at(-1, &mut buffer, 19, -17);"#,
    );
    let [attempt] = replay.attempts() else {
        panic!("read baseline retains one operation")
    };
    assert_eq!(attempt.operation_tag(), 6);
    assert_eq!(
        attempt
            .scalar_operands()
            .iter()
            .map(|operand| operand.value())
            .collect::<Vec<_>>(),
        vec![
            psi_checked_interpreter::FilesystemScalarOperandValue::U64(19),
            psi_checked_interpreter::FilesystemScalarOperandValue::I64(-17),
        ]
    );
    let [resolution] = attempt.mutable_byte_operand_resolutions() else {
        panic!("read baseline retains one resolution-time carrier")
    };
    let [carrier] = attempt.mutable_byte_operands() else {
        panic!("read baseline retains one provider carrier")
    };
    assert_eq!(resolution.operand_ordinal(), 1);
    assert_eq!(resolution.bytes().len(), 47);
    assert_eq!(resolution.bytes()[0], 11);
    assert_eq!(resolution.bytes()[23], 29);
    assert_eq!(resolution.bytes()[46], 173);
    assert_eq!(carrier.operand_ordinal(), 1);
    assert_eq!(resolution.bytes(), carrier.pre_bytes());
    assert_eq!(carrier.pre_bytes(), carrier.post_bytes());
    assert!(!replay.has_output_attempts());
}

fn assert_baseline_retains_unknown_descriptor_failure(
    label: &str,
    statement: &str,
    operation_tag: u16,
    scalar_values: &[psi_checked_interpreter::FilesystemScalarOperandValue],
) {
    let replay = unknown_descriptor_failure_baseline(label, statement);
    let [attempt] = replay.attempts() else {
        panic!("unknown-descriptor baseline retains one operation")
    };
    assert_eq!(attempt.operation_tag(), operation_tag);
    assert_eq!(
        attempt
            .scalar_operands()
            .iter()
            .map(|operand| operand.value())
            .collect::<Vec<_>>(),
        scalar_values
    );
    assert!(!replay.has_output_attempts());
}

fn unknown_descriptor_failure_baseline(
    label: &str,
    statements: &str,
) -> psi_checked_interpreter::FilesystemReplay {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let project = std::env::temp_dir().join(format!(
        "omega-review-baseline-{label}-{}-{sequence}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&project);
    std::fs::create_dir_all(&project).expect("create unknown-descriptor baseline fixture");
    std::fs::write(
        project.join("build.omg"),
        format!(
            r#"use omega::language::std::filesystem_host;

target windows_x64 {{ }}

machine build(builder: &mut Build)
reaches FilesystemHost
invokes FilesystemHost;
{{
    builder.application("review-baseline-{label}");
    {statements}
}}
"#
        ),
    )
    .expect("write unknown-descriptor baseline build");
    std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n")
        .expect("write unknown-descriptor baseline source");
    let compilation =
        omega_compiler::compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
            .expect("compile unknown-descriptor baseline fixture");
    let summary = compilation
        .build_observation_summary()
        .expect("unknown-descriptor failure publishes build observations");
    assert!(summary.filesystem_replay_verdict().is_complete());
    assert_eq!(summary.realized(), BuildObservationClass::Receipted);

    let limits = ReviewOnlyBaselineLimits::default();
    let replay =
        capture_verified_build_filesystem_replay_record(summary, replay_record_limits(limits))
            .expect("capture unknown-descriptor replay record")
            .expect("verified unknown-descriptor failure retains replay custody");
    let mut encoder = Encoder::bounded(limits.maximum_capsule_bytes);
    encode_replay_record_option(&mut encoder, Some(&replay))
        .expect("frame unknown-descriptor replay");
    let framed = encoder
        .finish()
        .expect("finish unknown-descriptor replay frame");
    let mut decoder = Decoder::new(&framed);
    let recovered = decode_replay_record_option(&mut decoder, limits)
        .expect("recover unknown-descriptor replay frame")
        .expect("unknown-descriptor replay frame is present");
    decoder
        .finish()
        .expect("unknown-descriptor replay consumes frame");
    let rehydrated = rehydrate_review_only_build_filesystem_replay_record(
        &recovered,
        replay_record_limits(limits),
    )
    .expect("unknown-descriptor baseline rehydrates through compiler replay custody");

    let _ = std::fs::remove_dir_all(project);
    rehydrated
}
