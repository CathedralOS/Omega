use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemRootedPathOperandResolution, BuildObservationClass,
};

fn absent_remove(path: &[u8]) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 9,
        provider: BuildFilesystemProvider::RealScoped,
        result: BuildFilesystemOperationResult::Scalar(-1),
        post_error: 2,
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Output,
            relative_path: path.to_vec(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Output,
            relative_path: path.to_vec(),
        }],
        logical_handle_inputs: Vec::new(),
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
        filesystem_operation_attempts: vec![
            absent_remove(b"missing-first.bin"),
            absent_remove(b"nested/missing-second.bin"),
        ],
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::Complete,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
    }
}

#[test]
fn failure_only_record_recovers_and_rehydrates_without_a_tree_entry() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&summary(), limits)
        .expect("exact absent removes encode")
        .expect("verified absent removes retain custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact absent removes recover");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact absent removes rehydrate through the failure-only constructor");

    assert!(replay.has_output_attempts());
    assert!(replay.output_entries().is_empty());
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![9, 9]
    );
    assert!(replay.attempts().iter().all(|attempt| {
        attempt.result()
            == Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
                -1,
            ))
            && attempt.post_error() == Some(2)
    }));
}

#[test]
fn failure_only_record_rejects_error_root_and_handoff_drift() {
    let limits = BuildFilesystemReplayRecordLimits::default();

    let mut changed_error = summary();
    changed_error.filesystem_operation_attempts[0].post_error = 13;
    assert!(
        capture_verified_build_filesystem_replay_record(&changed_error, limits).is_err(),
        "a different failure class is not an absent-remove receipt"
    );

    let mut changed_root = summary();
    changed_root.filesystem_operation_attempts[0].rooted_path_operand_resolutions[0].root =
        BuildFilesystemRoot::Source;
    assert!(capture_verified_build_filesystem_replay_record(&changed_root, limits).is_err());

    let mut handoff = summary();
    handoff
        .included_source_handoffs
        .push(crate::BuildIncludedSourceHandoff {
            relative_path: b"missing-first.bin".to_vec(),
            filesystem_attempt_ordinal: 1,
        });
    assert!(capture_verified_build_filesystem_replay_record(&handoff, limits).is_err());
}
