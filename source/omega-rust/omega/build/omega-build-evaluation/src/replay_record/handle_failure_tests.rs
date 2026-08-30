use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemLogicalHandleIdentity,
    BuildFilesystemLogicalHandleInput, BuildFilesystemLogicalHandleInputResolution,
    BuildFilesystemLogicalHandleKind, BuildObservationClass,
};

fn operand_free_unknown_descriptor_failure(operation_tag: u16) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
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

fn summary(operation_tag: u16) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![operand_free_unknown_descriptor_failure(operation_tag)],
        canonical_source_metadata_identity: None,
        source_inputs_replay_verified: true,
        operation_replay_verified: false,
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
    }
}

#[test]
fn operand_free_unknown_descriptor_failure_records_recover_and_rehydrate_exactly() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    for operation_tag in [8, 43, 44, 45] {
        let captured =
            capture_verified_build_filesystem_replay_record(&summary(operation_tag), limits)
                .expect("exact operand-free unknown-descriptor failure encodes")
                .expect("verified failure retains replay custody");
        let recovered =
            recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
                .expect("exact operand-free unknown-descriptor failure recovers");
        let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
            .expect("exact failure rehydrates through its typed constructor");

        assert!(!replay.has_output_attempts());
        assert!(replay.output_entries().is_empty());
        let [attempt] = replay.attempts() else {
            panic!("unknown-descriptor failure replay must retain one exact attempt")
        };
        assert_eq!(attempt.operation_tag(), operation_tag);
        assert_eq!(
            attempt.result(),
            Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1
            ))
        );
        assert_eq!(attempt.post_error(), Some(9));
        assert!(attempt.retired_logical_handles().is_empty());
    }
}

#[test]
fn operand_free_unknown_descriptor_failures_reject_shape_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut null = summary(43);
    null.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Null;
    assert!(capture_verified_build_filesystem_replay_record(&null, limits).is_err());

    let mut resolved = summary(44);
    resolved.filesystem_operation_attempts[0].logical_handle_inputs[0].resolution =
        BuildFilesystemLogicalHandleInputResolution::Resolved(
            BuildFilesystemLogicalHandleIdentity::new(1).unwrap(),
        );
    assert!(capture_verified_build_filesystem_replay_record(&resolved, limits).is_err());

    let mut changed_error = summary(45);
    changed_error.filesystem_operation_attempts[0].post_error = 13;
    assert!(capture_verified_build_filesystem_replay_record(&changed_error, limits).is_err());

    let mut retired = summary(8);
    retired.filesystem_operation_attempts[0]
        .retired_logical_handles
        .push(BuildFilesystemLogicalHandleIdentity::new(1).unwrap());
    assert!(capture_verified_build_filesystem_replay_record(&retired, limits).is_err());

    let mut handoff = summary(43);
    handoff
        .included_source_handoffs
        .push(crate::BuildIncludedSourceHandoff {
            relative_path: b"impossible.omg".to_vec(),
            filesystem_attempt_ordinal: 1,
        });
    assert!(capture_verified_build_filesystem_replay_record(&handoff, limits).is_err());
}
