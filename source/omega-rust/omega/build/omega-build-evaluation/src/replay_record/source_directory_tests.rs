use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemMutableI64Operand,
    BuildFilesystemMutableI64OperandResolution, BuildFilesystemObservedByteRegion,
    BuildFilesystemRootedPathOperandResolution, BuildFilesystemScalarOperand,
    BuildObservationClass,
};

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

fn source_directory_attempts() -> Vec<BuildFilesystemOperationAttempt> {
    let descriptor = identity(17);
    let mut open = attempt(2, BuildFilesystemOperationResult::LogicalHandle(descriptor));
    open.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(0),
    });
    open.rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Source,
            relative_path: b"entries".to_vec(),
        });
    open.authorized_paths.push(BuildFilesystemAuthorizedPath {
        operand_ordinal: 0,
        access: BuildFilesystemGrantAccess::Read,
        root: BuildFilesystemRoot::Source,
        relative_path: b"entries".to_vec(),
    });
    open.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        identity: descriptor,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });

    let mut read = attempt(23, BuildFilesystemOperationResult::Scalar(3));
    read.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 2,
        value: BuildFilesystemScalarOperandValue::U64(6),
    });
    read.observed_byte_regions
        .push(BuildFilesystemObservedByteRegion {
            output_operand_ordinal: 1,
            kind: BuildFilesystemObservedByteRegionKind::DirectoryRecords,
            offset: 0,
            length: 3,
        });
    read.mutable_byte_operand_resolutions
        .push(BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: vec![1, 2, 3, 4, 5, 6],
        });
    read.mutable_byte_operands
        .push(BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: vec![10, 11, 12, 13, 14, 15],
            post_bytes: vec![b'a', b'b', b'c', 13, 14, 15],
        });
    read.mutable_i64_operand_resolutions
        .push(BuildFilesystemMutableI64OperandResolution {
            operand_ordinal: 3,
            value: 0,
        });
    read.mutable_i64_operands
        .push(BuildFilesystemMutableI64Operand {
            operand_ordinal: 3,
            pre_value: 0,
            post_value: 3,
        });
    read.logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(descriptor),
        });

    let mut close = attempt(8, BuildFilesystemOperationResult::Scalar(0));
    close
        .logical_handle_inputs
        .push(BuildFilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: BuildFilesystemLogicalHandleKind::Descriptor,
            resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(descriptor),
        });
    close.retired_logical_handles.push(descriptor);
    vec![open, read, close]
}

fn summary() -> BuildObservationSummary {
    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: source_directory_attempts(),
        canonical_source_metadata_identity: None,
        source_inputs_replay_verified: true,
        operation_replay_verified: true,
        included_source_handoffs: Vec::new(),
        staged_output_tree: None,
    }
}

#[test]
fn source_directory_record_recovers_exact_byte_and_cursor_carriers() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&summary(), limits)
        .expect("directory record encodes")
        .expect("verified Source directory record retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("directory record recovers");
    let replay = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("directory record rehydrates");
    assert_eq!(
        replay
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 23, 8]
    );
    let read = &replay.attempts()[1];
    assert_eq!(
        read.mutable_byte_operand_resolutions()[0].bytes(),
        &[1, 2, 3, 4, 5, 6]
    );
    assert_eq!(
        read.mutable_byte_operands()[0].post_bytes(),
        &[b'a', b'b', b'c', 13, 14, 15]
    );
    assert_eq!(read.mutable_i64_operand_resolutions()[0].value(), 0);
    assert_eq!(read.mutable_i64_operands()[0].post_value(), 3);
}

#[test]
fn source_directory_record_rejects_tampered_cursor_and_tail_lanes() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut cursor = summary();
    cursor.filesystem_operation_attempts[1].mutable_i64_operands[0].operand_ordinal = 2;
    assert!(capture_verified_build_filesystem_replay_record(&cursor, limits).is_err());

    let mut tail = summary();
    tail.filesystem_operation_attempts[1].mutable_byte_operands[0].post_bytes[5] ^= 1;
    assert!(capture_verified_build_filesystem_replay_record(&tail, limits).is_err());
}
