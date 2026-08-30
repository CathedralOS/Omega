use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemRootedPathOperandResolution,
    BuildFilesystemScalarOperand, BuildIncludedSourceHandoff, BuildObservationClass,
};

const OUTPUT_PATH: &[u8] = b"generated.omg";

fn identity(value: u64) -> BuildFilesystemLogicalHandleIdentity {
    BuildFilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
}

fn attempt(
    operation_tag: u16,
    result: BuildFilesystemOperationResult,
) -> BuildFilesystemOperationAttempt {
    BuildFilesystemOperationAttempt {
        operation_tag,
        provider: BuildFilesystemProvider::RealScoped,
        result,
        post_error: 0,
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

fn output_file_attempts() -> Vec<BuildFilesystemOperationAttempt> {
    let output_identity = identity(73);
    let mut create = attempt(
        1,
        BuildFilesystemOperationResult::LogicalHandle(output_identity),
    );
    create.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(
            psi_checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE,
        ),
    });
    create
        .rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Output,
            relative_path: OUTPUT_PATH.to_vec(),
        });
    create.authorized_paths.push(BuildFilesystemAuthorizedPath {
        operand_ordinal: 0,
        access: BuildFilesystemGrantAccess::Write,
        root: BuildFilesystemRoot::Output,
        relative_path: OUTPUT_PATH.to_vec(),
    });
    create.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        identity: output_identity,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });

    let mut close = attempt(8, BuildFilesystemOperationResult::Scalar(0));
    close
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(output_identity),
        });
    close.retired_logical_handles.push(output_identity);
    vec![create, close]
}

pub(super) fn output_only_summary(handoff_ordinal: u64) -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: output_file_attempts(),
        canonical_source_metadata_identity: None,
        filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
            BuildFilesystemReplayDisposition::Complete,
        ),
        included_source_handoffs: vec![BuildIncludedSourceHandoff {
            relative_path: OUTPUT_PATH.to_vec(),
            filesystem_attempt_ordinal: handoff_ordinal,
        }],
        staged_output_tree: None,
        build_log: Vec::new(),
    }
}

#[test]
fn output_only_record_recovers_and_rehydrates_from_attempt_zero() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&output_only_summary(2), limits)
        .expect("exact Output-only record encodes")
        .expect("verified Output-only record retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact Output-only record recovers");
    assert_eq!(recovered, captured);

    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact Output-only record rehydrates through the no-Source constructor");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![1, 8]
    );
    assert_eq!(replay.output_files()[0].output_relative_path(), OUTPUT_PATH);
    let [included] = replay.expected_included_sources() else {
        panic!("Output-only replay retains its one generated-source handoff")
    };
    assert_eq!(included.relative_path(), OUTPUT_PATH);
    assert_eq!(included.filesystem_attempt_ordinal(), 2);
}

#[test]
fn output_only_record_rejects_handoff_before_its_attempt_zero_file_closes() {
    assert!(
        capture_verified_build_filesystem_replay_record(
            &output_only_summary(1),
            BuildFilesystemReplayRecordLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn output_only_record_rejects_a_non_output_prefix() {
    let mut summary = output_only_summary(3);
    summary
        .filesystem_operation_attempts
        .insert(0, attempt(10, BuildFilesystemOperationResult::Scalar(0)));
    assert!(
        capture_verified_build_filesystem_replay_record(
            &summary,
            BuildFilesystemReplayRecordLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn output_only_record_rejects_empty_attempts_and_tampered_output_shape() {
    assert!(validate_first_rung(&[]).is_err());

    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&output_only_summary(2), limits)
        .expect("exact Output-only record encodes")
        .expect("verified Output-only record retains custody");
    let mut create_prefix = Vec::new();
    create_prefix.extend_from_slice(&1u16.to_le_bytes());
    create_prefix.push(2);
    create_prefix.push(1);
    create_prefix.extend_from_slice(&73u64.to_le_bytes());
    create_prefix.extend_from_slice(&0i32.to_le_bytes());
    let offsets = captured
        .canonical_bytes()
        .windows(create_prefix.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == create_prefix).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "Output create framing is unique");

    let mut tampered = captured.canonical_bytes().to_vec();
    tampered[offsets[0] + 2] = 1;
    assert!(recover_review_only_build_filesystem_replay_record(&tampered, limits).is_err());
}
