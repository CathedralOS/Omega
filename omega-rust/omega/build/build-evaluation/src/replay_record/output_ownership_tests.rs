use super::*;
use crate::{
    BuildFilesystemAuthorizedPath, BuildFilesystemLogicalHandleIdentity,
    BuildFilesystemLogicalHandleInput, BuildFilesystemOperationAttempt,
    BuildFilesystemScalarOperand,
};

fn identity(value: u64) -> BuildFilesystemLogicalHandleIdentity {
    BuildFilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
}

fn ownership_attempt(
    uid: i32,
    gid: i32,
    result: i64,
    post_error: i32,
) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 49,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
        result: BuildFilesystemOperationResult::Scalar(result),
        post_error,
        scalar_operands: vec![
            BuildFilesystemScalarOperand {
                operand_ordinal: 1,
                value: BuildFilesystemScalarOperandValue::I32(uid),
            },
            BuildFilesystemScalarOperand {
                operand_ordinal: 2,
                value: BuildFilesystemScalarOperandValue::I32(gid),
            },
        ],
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
        logical_handle_inputs: vec![BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(identity(73)),
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn ownership_summary() -> BuildObservationSummary {
    let mut summary = super::output_only_tests::output_only_summary(5);
    let close = summary.filesystem_operation_attempts.pop().unwrap();
    summary.filesystem_operation_attempts.extend([
        ownership_attempt(-1, -1, 0, 0),
        ownership_attempt(0, 0, -1, 1),
        ownership_attempt(-1, -1, 0, 1),
    ]);
    let mut close = close;
    close.post_error = 1;
    summary.filesystem_operation_attempts.push(close);
    summary
}

#[test]
fn record_recovers_exact_success_failure_and_carried_error_sequence() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let summary = ownership_summary();
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact ownership replay encodes")
        .expect("verified ownership replay retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact ownership replay recovers");
    let rehydrated = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact ownership replay rehydrates");

    assert_eq!(
        rehydrated
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![1, 49, 49, 49, 8]
    );
    let outputs = rehydrated.output_files();
    let operations = outputs[0].operations();
    assert!(matches!(
        operations,
        [
            checked_interpreter::FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(success),
            checked_interpreter::FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(failure),
            checked_interpreter::FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(carried),
        ] if success.uid() == -1
            && success.gid() == -1
            && success.result() == 0
            && success.post_error() == 0
            && failure.uid() == 0
            && failure.gid() == 0
            && failure.result() == -1
            && failure.post_error() == 1
            && carried.result() == 0
            && carried.post_error() == 1
    ));
    assert_eq!(rehydrated.attempts().last().unwrap().post_error(), Some(1));
}

#[test]
fn capture_rejects_ownership_scalar_outcome_lineage_and_side_lane_drift_atomically() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let exact = ownership_summary();
    let exact_record = capture_verified_build_filesystem_replay_record(&exact, limits)
        .unwrap()
        .unwrap();

    let mut changed = exact.clone();
    changed.filesystem_operation_attempts[2].scalar_operands[1].operand_ordinal = 1;
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let mut changed = exact.clone();
    changed.filesystem_operation_attempts[2].result = BuildFilesystemOperationResult::Scalar(1);
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let mut changed = exact.clone();
    changed.filesystem_operation_attempts[2].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(identity(74));
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let mut changed = exact.clone();
    changed.filesystem_operation_attempts[2]
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Output,
            relative_path: b"generated.omg".to_vec(),
        });
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let mut changed = exact.clone();
    changed.filesystem_operation_attempts.swap(3, 4);
    assert!(capture_verified_build_filesystem_replay_record(&changed, limits).is_err());

    let recaptured = capture_verified_build_filesystem_replay_record(&exact, limits)
        .unwrap()
        .unwrap();
    assert_eq!(recaptured, exact_record);
}
