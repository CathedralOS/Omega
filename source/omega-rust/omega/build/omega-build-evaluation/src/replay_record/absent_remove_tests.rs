use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemRootedPathOperandResolution, BuildObservationClass,
};

fn absent_remove(operation_tag: u16, path: &[u8]) -> BuildFilesystemOperationAttempt {
    assert!(matches!(operation_tag, 9 | 12));
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
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
            absent_remove(12, b"nested/missing-second"),
            absent_remove(9, b"missing-first.bin"),
        ],
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::Complete,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

fn source_prefixed_directory_replay() -> psi_checked_interpreter::FilesystemReplay {
    let read = psi_checked_interpreter::FilesystemReplayReadRecord::new(
        psi_checked_interpreter::FilesystemReplayReadKind::Sequential,
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("empty Source read is canonical");
    let chain = psi_checked_interpreter::FilesystemSourceReadChainReplayRecord::new(
        crate::BUILD_SOURCE_ROOT_IDENTITY,
        b"inputs/table.txt".to_vec(),
        17,
        0,
        vec![read],
        0,
    )
    .expect("Source chain is canonical");
    let source = psi_checked_interpreter::FilesystemSourceInputReplayRecord::new(vec![
        psi_checked_interpreter::FilesystemSourceInputReplayEventRecord::ReadChain(chain),
    ])
    .expect("Source input is nonempty");
    let remove = psi_checked_interpreter::FilesystemOutputAbsentRemoveReplayRecord::new(
        psi_checked_interpreter::FilesystemOutputAbsentRemoveKind::Directory,
        crate::BUILD_OUTPUT_ROOT_IDENTITY,
        b"missing-directory".to_vec(),
    )
    .expect("absent directory removal is canonical");
    let record = psi_checked_interpreter::FilesystemInputOutputAbsentRemovesReplayRecord::new(
        Some(source),
        vec![remove],
    )
    .expect("Source-prefixed absent directory removal is canonical");
    psi_checked_interpreter::FilesystemReplay::from_input_output_absent_removes_record(record)
        .expect("Source-prefixed absent directory replay is bounded")
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
        vec![12, 9]
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
fn directory_only_record_recovers_from_attempt_zero() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut directory_only = summary();
    directory_only.filesystem_operation_attempts.truncate(1);
    let captured = capture_verified_build_filesystem_replay_record(&directory_only, limits)
        .expect("exact absent directory removal encodes")
        .expect("verified absent directory removal retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("absent directory removal recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("absent directory removal rehydrates");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![12]
    );
}

#[test]
fn source_prefix_classifier_stops_before_directory_removal() {
    let replay = source_prefixed_directory_replay();
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 12]
    );
    assert_eq!(
        crate::source_input_replay_prefix_end(replay.attempts()),
        Some(3)
    );
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

    let mut changed_operation = summary();
    changed_operation.filesystem_operation_attempts[1].operation_tag = 11;
    assert!(capture_verified_build_filesystem_replay_record(&changed_operation, limits).is_err());

    let mut handoff = summary();
    handoff
        .included_source_handoffs
        .push(crate::BuildIncludedSourceHandoff {
            relative_path: b"missing-first.bin".to_vec(),
            filesystem_attempt_ordinal: 1,
        });
    assert!(capture_verified_build_filesystem_replay_record(&handoff, limits).is_err());
}
