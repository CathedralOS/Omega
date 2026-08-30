use super::{
    BuildFilesystemReplayRecordLimits, capture_verified_build_filesystem_replay_record,
    native_mutation_failure_tests::{
        lock_file_ex_summary, set_file_time_summary, unlock_file_summary,
    },
    recover_review_only_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};
use crate::{
    BuildFilesystemByteOperand, BuildFilesystemOperationAttempt, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildObservationSummary,
};

fn get_last_error() -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 35,
        provider: BuildFilesystemProvider::RealScoped,
        result: BuildFilesystemOperationResult::Scalar(6),
        post_error: 6,
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

fn with_error_read(mut summary: BuildObservationSummary) -> BuildObservationSummary {
    summary.filesystem_operation_attempts.push(get_last_error());
    summary
}

#[test]
fn ordered_native_mutation_and_last_error_round_trip_all_variants() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for summary in [
        with_error_read(set_file_time_summary()),
        with_error_read(lock_file_ex_summary()),
        with_error_read(unlock_file_summary()),
    ] {
        let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
            .expect("exact ordered native error-state sequence encodes")
            .expect("verified ordered sequence retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("ordered sequence recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("ordered sequence rehydrates through its typed constructor");
        let [mutation, error_read] = replay.attempts() else {
            panic!("ordered sequence retains exactly two attempts")
        };
        assert!(matches!(mutation.operation_tag(), 32 | 33 | 34));
        assert_eq!(error_read.operation_tag(), 35);
        assert_eq!(
            error_read.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                6
            ))
        );
        assert_eq!(error_read.post_error(), Some(6));
        assert!(!replay.has_output_attempts());
    }
}

#[test]
fn error_state_read_is_admitted_only_as_the_exact_immediate_second_row() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut standalone = unlock_file_summary();
    standalone.filesystem_operation_attempts = vec![get_last_error()];
    assert!(capture_verified_build_filesystem_replay_record(&standalone, limits).is_err());

    let mut reordered = with_error_read(unlock_file_summary());
    reordered.filesystem_operation_attempts.swap(0, 1);
    assert!(capture_verified_build_filesystem_replay_record(&reordered, limits).is_err());

    let mut changed_result = with_error_read(unlock_file_summary());
    changed_result.filesystem_operation_attempts[1].result =
        BuildFilesystemOperationResult::Scalar(5);
    assert!(capture_verified_build_filesystem_replay_record(&changed_result, limits).is_err());

    let mut invented_operand = with_error_read(unlock_file_summary());
    invented_operand.filesystem_operation_attempts[1]
        .byte_operands
        .push(BuildFilesystemByteOperand {
            operand_ordinal: 0,
            bytes: vec![0],
        });
    assert!(capture_verified_build_filesystem_replay_record(&invented_operand, limits).is_err());
}
