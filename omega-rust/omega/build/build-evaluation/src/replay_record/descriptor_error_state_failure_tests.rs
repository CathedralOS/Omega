use super::{
    BuildFilesystemReplayRecordLimits, capture_verified_build_filesystem_replay_record,
    handle_failure_tests::{
        summary, unknown_descriptor_at_summary, unknown_descriptor_get_osfhandle_summary,
        unknown_descriptor_read_file_metadata_summary, unknown_descriptor_read_summary,
        unknown_descriptor_seek_summary, unknown_descriptor_set_file_times_summary,
        unknown_descriptor_write_payload_summary, unknown_descriptor_write_summary,
        unknown_native_handle_close_summary,
    },
    read_dir_failure_tests, recover_review_only_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};
use crate::{
    BuildFilesystemByteOperand, BuildFilesystemOperationAttempt, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemScalarOperandValue, BuildObservationSummary,
};

fn errno() -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 50,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
        result: BuildFilesystemOperationResult::Scalar(9),
        post_error: 9,
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn with_errno(mut summary: BuildObservationSummary) -> BuildObservationSummary {
    summary.filesystem_operation_attempts.push(errno());
    summary
}

#[test]
fn operand_free_descriptor_failure_and_errno_round_trip_all_variants() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for operation_tag in [8, 43, 44, 45] {
        let captured = capture_verified_build_filesystem_replay_record(
            &with_errno(summary(operation_tag)),
            limits,
        )
        .expect("exact descriptor error-state sequence encodes")
        .expect("verified descriptor error-state sequence retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("descriptor error-state sequence recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("descriptor error-state sequence rehydrates through its typed constructor");
        let [operation, errno] = replay.attempts() else {
            panic!("descriptor error-state sequence retains exactly two attempts")
        };
        assert_eq!(operation.operation_tag(), operation_tag);
        assert_eq!(errno.operation_tag(), 50);
        assert_eq!(
            errno.result(),
            Some(checked_interpreter::FilesystemOperationResult::Scalar(9))
        );
        assert_eq!(errno.post_error(), Some(9));
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn every_exact_bad_descriptor_failure_and_errno_round_trip_together() {
    use BuildFilesystemScalarOperandValue::{I32, I64, U32};

    let cases = vec![
        (10, unknown_descriptor_seek_summary(-17, 2)),
        (14, unknown_descriptor_at_summary(14, b"child".to_vec(), 3)),
        (15, unknown_descriptor_at_summary(15, b"child".to_vec(), 4)),
        (23, read_dir_failure_tests::summary(vec![0x5a; 47], 31, -19)),
        (17, unknown_descriptor_write_summary(17, &[U32(0o640)])),
        (41, unknown_descriptor_write_summary(41, &[I64(37)])),
        (46, unknown_descriptor_write_summary(46, &[I32(2)])),
        (
            49,
            unknown_descriptor_write_summary(49, &[I32(1000), I32(1001)]),
        ),
        (
            42,
            unknown_descriptor_set_file_times_summary(vec![0x2a; 32]),
        ),
        (
            4,
            unknown_descriptor_read_summary(
                4,
                vec![0x31; 16],
                &[crate::BuildFilesystemScalarOperandValue::U64(16)],
            ),
        ),
        (
            6,
            unknown_descriptor_read_summary(
                6,
                vec![0x32; 16],
                &[crate::BuildFilesystemScalarOperandValue::U64(16), I64(-7)],
            ),
        ),
        (
            5,
            unknown_descriptor_write_payload_summary(5, b"payload".to_vec(), None),
        ),
        (
            7,
            unknown_descriptor_write_payload_summary(7, b"payload".to_vec(), Some(-7)),
        ),
        (
            39,
            unknown_descriptor_read_file_metadata_summary(vec![0x44; 144]),
        ),
    ];
    let limits = BuildFilesystemReplayRecordLimits::default();

    for (operation_tag, summary) in cases {
        let summary = with_errno(summary);
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact bad-descriptor failure and immediate errno encode")
            .expect("verified bad-descriptor sequence retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("bad-descriptor sequence recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("bad-descriptor sequence rehydrates through typed records");
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            [operation_tag, 50]
        );
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn errno_pair_excludes_failures_that_do_not_establish_bad_descriptor_state() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for summary in [
        unknown_descriptor_get_osfhandle_summary(),
        unknown_native_handle_close_summary(),
    ] {
        assert!(
            capture_verified_build_filesystem_replay_record(&with_errno(summary), limits).is_err()
        );
    }
}

#[test]
fn errno_is_admitted_only_as_the_exact_immediate_second_row() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut standalone = summary(8);
    standalone.filesystem_operation_attempts = vec![errno()];
    assert!(capture_verified_build_filesystem_replay_record(&standalone, limits).is_err());

    let mut reordered = with_errno(summary(43));
    reordered.filesystem_operation_attempts.swap(0, 1);
    assert!(capture_verified_build_filesystem_replay_record(&reordered, limits).is_err());

    let mut changed_result = with_errno(summary(44));
    changed_result.filesystem_operation_attempts[1].result =
        BuildFilesystemOperationResult::Scalar(6);
    assert!(capture_verified_build_filesystem_replay_record(&changed_result, limits).is_err());

    let mut invented_operand = with_errno(summary(45));
    invented_operand.filesystem_operation_attempts[1]
        .byte_operands
        .push(BuildFilesystemByteOperand {
            operand_ordinal: 0,
            bytes: vec![0],
        });
    assert!(capture_verified_build_filesystem_replay_record(&invented_operand, limits).is_err());
}
