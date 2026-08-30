use super::{
    BuildFilesystemReplayRecordLimits, capture_verified_build_filesystem_replay_record,
    handle_failure_tests::summary, recover_review_only_build_filesystem_replay_record,
    rehydrate_review_only_build_filesystem_replay_record,
};
use crate::{
    BuildFilesystemByteOperand, BuildFilesystemOperationAttempt, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildObservationSummary,
};

fn errno() -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 50,
        provider: BuildFilesystemProvider::RealScoped,
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
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                9
            ))
        );
        assert_eq!(errno.post_error(), Some(9));
        assert!(!replay.has_output_attempts());
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
