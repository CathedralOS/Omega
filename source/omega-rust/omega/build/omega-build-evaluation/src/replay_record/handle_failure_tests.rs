use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemLogicalHandleIdentity,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildObservationClass,
};

fn unknown_descriptor_close() -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 8,
        provider: BuildFilesystemProvider::RealScoped,
        result: BuildFilesystemOperationResult::Scalar(-1),
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
        logical_handle_inputs: vec![BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn summary() -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![unknown_descriptor_close()],
        canonical_source_metadata_identity: None,
        source_inputs_replay_verified: true,
        operation_replay_verified: false,
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
    }
}

#[test]
fn unknown_descriptor_close_record_recovers_and_rehydrates_exact_failure() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&summary(), limits)
        .expect("exact unknown-descriptor close encodes")
        .expect("verified failure retains replay custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact unknown-descriptor close recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact unknown-descriptor close rehydrates through its typed constructor");

    assert!(!replay.has_output_attempts());
    assert!(replay.output_entries().is_empty());
    let [attempt] = replay.attempts() else {
        panic!("unknown-descriptor close replay must retain one exact attempt")
    };
    assert_eq!(attempt.operation_tag(), 8);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            -1,
        ))
    );
    assert_eq!(attempt.post_error(), Some(9));
    assert!(attempt.retired_logical_handles().is_empty());
}

#[test]
fn unknown_descriptor_close_rejects_null_resolved_and_side_lane_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut null = summary();
    null.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Null;
    assert!(capture_verified_build_filesystem_replay_record(&null, limits).is_err());

    let mut resolved = summary();
    resolved.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(
            BuildFilesystemLogicalHandleIdentity::new(1).unwrap(),
        );
    assert!(capture_verified_build_filesystem_replay_record(&resolved, limits).is_err());

    let mut changed_error = summary();
    changed_error.filesystem_operation_attempts[0].post_error = 13;
    assert!(capture_verified_build_filesystem_replay_record(&changed_error, limits).is_err());

    let mut retired = summary();
    retired.filesystem_operation_attempts[0]
        .retired_logical_handles
        .push(BuildFilesystemLogicalHandleIdentity::new(1).unwrap());
    assert!(capture_verified_build_filesystem_replay_record(&retired, limits).is_err());

    let mut handoff = summary();
    handoff
        .included_source_handoffs
        .push(crate::BuildIncludedSourceHandoff {
            relative_path: b"impossible.omg".to_vec(),
            filesystem_attempt_ordinal: 1,
        });
    assert!(capture_verified_build_filesystem_replay_record(&handoff, limits).is_err());
}
