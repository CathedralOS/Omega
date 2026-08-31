use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath, BuildFilesystemGrantRefusal,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildObservationClass,
};

fn exact_attempt() -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag: 1,
        provider: BuildFilesystemProvider::RealScoped,
        observation_class: BuildFilesystemOperationObservationClass::Receipted,
        result: BuildFilesystemOperationResult::Scalar(-1),
        post_error: 13,
        scalar_operands: vec![BuildFilesystemScalarOperand {
            operand_ordinal: 1,
            value: BuildFilesystemScalarOperandValue::I32(438),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Source,
            relative_path: b"blocked.bin".to_vec(),
        }],
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
        grant_refusals: vec![BuildFilesystemGrantRefusal {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            reason: BuildFilesystemGrantRefusalReason::OutsideGrantedRoots,
        }],
    }
}

fn exact_remove_attempt() -> BuildFilesystemOperationAttempt {
    let mut attempt = exact_attempt();
    attempt.operation_tag = 9;
    attempt.scalar_operands.clear();
    attempt
}

fn summary(attempts: Vec<BuildFilesystemOperationAttempt>) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: attempts,
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::Complete,
        ),
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

fn capture_rejects(attempt: BuildFilesystemOperationAttempt) {
    assert!(
        capture_verified_build_filesystem_replay_record(
            &summary(vec![attempt]),
            BuildFilesystemReplayRecordLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn refused_source_write_recovers_and_rehydrates_without_output() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured =
        capture_verified_build_filesystem_replay_record(&summary(vec![exact_attempt()]), limits)
            .expect("exact refused Source write encodes")
            .expect("verified refused Source write retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact refused Source write recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact refused Source write rehydrates");

    assert!(!replay.has_output_attempts());
    assert!(replay.output_entries().is_empty());
    let [attempt] = replay.attempts() else {
        panic!("refused Source write replay must retain one attempt")
    };
    assert_eq!(attempt.operation_tag(), 1);
    assert_eq!(
        attempt.result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            -1
        ))
    );
    assert_eq!(attempt.post_error(), Some(13));
    assert_eq!(
        attempt.rooted_path_operand_resolutions()[0].relative_path(),
        b"blocked.bin"
    );
    assert_eq!(attempt.grant_refusals().len(), 1);
}

#[test]
fn refused_source_remove_recovers_as_policy_not_output_mutation() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(
        &summary(vec![exact_remove_attempt()]),
        limits,
    )
    .expect("exact refused Source remove encodes")
    .expect("verified refused Source remove retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact refused Source remove recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact refused Source remove rehydrates");

    assert!(!replay.has_output_attempts());
    assert!(replay.output_entries().is_empty());
    let [attempt] = replay.attempts() else {
        panic!("refused Source remove replay must retain one attempt")
    };
    assert_eq!(attempt.operation_tag(), 9);
    assert!(attempt.scalar_operands().is_empty());
    assert_eq!(attempt.rooted_path_operand_resolutions().len(), 1);
    assert_eq!(attempt.grant_refusals().len(), 1);
}

#[test]
fn refused_source_write_codec_rejects_omission_duplication_and_framing_mutation() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    assert!(capture_verified_build_filesystem_replay_record(&summary(Vec::new()), limits).is_err());
    assert!(
        capture_verified_build_filesystem_replay_record(
            &summary(vec![exact_attempt(), exact_attempt()]),
            limits,
        )
        .is_err()
    );

    let captured =
        capture_verified_build_filesystem_replay_record(&summary(vec![exact_attempt()]), limits)
            .unwrap()
            .unwrap();
    let mut truncated = captured.canonical_bytes().to_vec();
    truncated.pop();
    assert!(recover_review_only_build_filesystem_replay_record(&truncated, limits).is_err());
    let mut trailing = captured.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(recover_review_only_build_filesystem_replay_record(&trailing, limits).is_err());

    let mut prior_version = captured.canonical_bytes().to_vec();
    prior_version[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&54u16.to_le_bytes());
    assert!(recover_review_only_build_filesystem_replay_record(&prior_version, limits).is_err());
}

#[test]
fn refused_source_write_codec_rejects_independent_semantic_mutations() {
    let mut changed = exact_attempt();
    changed.operation_tag = 2;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.provider = BuildFilesystemProvider::Virtual;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.rooted_path_operand_resolutions[0].root = BuildFilesystemRoot::Output;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.rooted_path_operand_resolutions[0].relative_path = b"other.bin".to_vec();
    // A different canonical path is a different valid denial, not a malformed
    // record. Bind its identity by requiring the recovered path to match.
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&summary(vec![changed]), limits)
        .unwrap()
        .unwrap();
    let replay = rehydrate_review_only_build_filesystem_replay_record(&captured, limits).unwrap();
    assert_eq!(
        replay.attempts()[0].rooted_path_operand_resolutions()[0].relative_path(),
        b"other.bin"
    );

    let mut changed = exact_attempt();
    changed.scalar_operands[0].value = BuildFilesystemScalarOperandValue::I32(420);
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.result = BuildFilesystemOperationResult::Scalar(0);
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.post_error = 2;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.grant_refusals[0].operand_ordinal = 1;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.grant_refusals[0].access = BuildFilesystemGrantAccess::Read;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.grant_refusals[0].reason = BuildFilesystemGrantRefusalReason::Unresolvable;
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed.grant_refusals.clear();
    capture_rejects(changed);

    let mut changed = exact_attempt();
    let duplicate = changed.grant_refusals[0];
    changed.grant_refusals.push(duplicate);
    capture_rejects(changed);

    let mut changed = exact_attempt();
    changed
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Source,
            relative_path: b"blocked.bin".to_vec(),
        });
    capture_rejects(changed);
}

#[test]
fn refused_source_remove_rejects_create_mode_and_output_authorization_shapes() {
    let mut changed = exact_remove_attempt();
    changed.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(438),
    });
    capture_rejects(changed);

    let mut changed = exact_remove_attempt();
    changed.rooted_path_operand_resolutions[0].root = BuildFilesystemRoot::Output;
    changed.grant_refusals.clear();
    changed
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Output,
            relative_path: b"blocked.bin".to_vec(),
        });
    capture_rejects(changed);
}
