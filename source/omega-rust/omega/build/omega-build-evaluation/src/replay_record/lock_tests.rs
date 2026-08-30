use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath, BuildFilesystemByteOperand,
    BuildFilesystemLogicalHandleIdentity, BuildFilesystemLogicalHandleInput,
    BuildFilesystemLogicalHandleOutput, BuildFilesystemMutableByteOperand,
    BuildFilesystemMutableByteOperandResolution, BuildFilesystemObservedByteRegion,
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
        observation_class: crate::BuildFilesystemOperationObservationClass::Receipted,
        result,
        post_error: 0,
        scalar_operands: Vec::new(),
        byte_operands: Vec::<BuildFilesystemByteOperand>::new(),
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

fn descriptor_input(
    identity: BuildFilesystemLogicalHandleIdentity,
) -> BuildFilesystemLogicalHandleInput {
    BuildFilesystemLogicalHandleInput {
        operand_ordinal: 0,
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(identity),
    }
}

fn replay_summary() -> BuildObservationSummary {
    let source_identity = identity(1);
    let output_identity = identity(2);

    let mut source_open = attempt(
        2,
        BuildFilesystemOperationResult::LogicalHandle(source_identity),
    );
    source_open
        .scalar_operands
        .push(BuildFilesystemScalarOperand {
            operand_ordinal: 1,
            value: BuildFilesystemScalarOperandValue::I32(0),
        });
    source_open
        .rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Source,
            relative_path: b"main.omg".to_vec(),
        });
    source_open
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Read,
            root: BuildFilesystemRoot::Source,
            relative_path: b"main.omg".to_vec(),
        });
    source_open.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        identity: source_identity,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });

    let mut source_read = attempt(4, BuildFilesystemOperationResult::Scalar(0));
    source_read
        .scalar_operands
        .push(BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::U64(0),
        });
    source_read.mutable_byte_operand_resolutions.push(
        BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: Vec::new(),
        },
    );
    source_read
        .mutable_byte_operands
        .push(BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: Vec::new(),
            post_bytes: Vec::new(),
        });
    source_read
        .observed_byte_regions
        .push(BuildFilesystemObservedByteRegion {
            output_operand_ordinal: 1,
            kind: BuildFilesystemObservedByteRegionKind::SequentialFileRead,
            offset: 0,
            length: 0,
        });
    source_read
        .logical_handle_inputs
        .push(descriptor_input(source_identity));

    let mut source_close = attempt(8, BuildFilesystemOperationResult::Scalar(0));
    source_close
        .logical_handle_inputs
        .push(descriptor_input(source_identity));
    source_close.retired_logical_handles.push(source_identity);

    let mut output_create = attempt(
        1,
        BuildFilesystemOperationResult::LogicalHandle(output_identity),
    );
    output_create
        .scalar_operands
        .push(BuildFilesystemScalarOperand {
            operand_ordinal: 1,
            value: BuildFilesystemScalarOperandValue::I32(
                psi_checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE,
            ),
        });
    output_create.rooted_path_operand_resolutions.push(
        BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Output,
            relative_path: b"locked.bin".to_vec(),
        },
    );
    output_create
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Output,
            relative_path: b"locked.bin".to_vec(),
        });
    output_create.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        identity: output_identity,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });

    let mut acquire = attempt(46, BuildFilesystemOperationResult::Scalar(0));
    acquire.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(6),
    });
    acquire
        .logical_handle_inputs
        .push(descriptor_input(output_identity));

    let mut release = attempt(46, BuildFilesystemOperationResult::Scalar(0));
    release.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(8),
    });
    release
        .logical_handle_inputs
        .push(descriptor_input(output_identity));

    let mut output_close = attempt(8, BuildFilesystemOperationResult::Scalar(0));
    output_close
        .logical_handle_inputs
        .push(descriptor_input(output_identity));
    output_close.retired_logical_handles.push(output_identity);

    BuildObservationSummary {
        schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
        ceiling: BuildObservationClass::Volatile,
        realized: BuildObservationClass::Receipted,
        filesystem_operation_schema_version:
            psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
        filesystem_operation_attempts: vec![
            source_open,
            source_read,
            source_close,
            output_create,
            acquire,
            release,
            output_close,
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

fn exact_attempt_prefix(operation: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&46u16.to_le_bytes());
    bytes.push(2);
    bytes.push(0);
    bytes.push(0);
    bytes.extend_from_slice(&0i64.to_le_bytes());
    bytes.extend_from_slice(&0i32.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.push(1);
    bytes.push(0);
    bytes.extend_from_slice(&operation.to_le_bytes());
    bytes
}

fn hard_link_attempt(
    operation_tag: u16,
    existing: &[u8],
    output: &[u8],
) -> BuildFilesystemOperationAttempt {
    let windows = operation_tag == 27;
    let mut attempt = attempt(
        operation_tag,
        BuildFilesystemOperationResult::Scalar(if windows { 1 } else { 0 }),
    );
    if windows {
        attempt.scalar_operands.push(BuildFilesystemScalarOperand {
            operand_ordinal: 2,
            value: BuildFilesystemScalarOperandValue::I64(0),
        });
    }
    let rooted =
        |operand_ordinal, relative_path: &[u8]| BuildFilesystemRootedPathOperandResolution {
            operand_ordinal,
            root: BuildFilesystemRoot::Output,
            relative_path: relative_path.to_vec(),
        };
    if windows {
        attempt
            .rooted_path_operand_resolutions
            .extend([rooted(0, output), rooted(1, existing)]);
        attempt.authorized_paths.extend([
            BuildFilesystemAuthorizedPath {
                operand_ordinal: 1,
                access: BuildFilesystemGrantAccess::Write,
                root: BuildFilesystemRoot::Output,
                relative_path: existing.to_vec(),
            },
            BuildFilesystemAuthorizedPath {
                operand_ordinal: 0,
                access: BuildFilesystemGrantAccess::Write,
                root: BuildFilesystemRoot::Output,
                relative_path: output.to_vec(),
            },
        ]);
    } else {
        attempt
            .rooted_path_operand_resolutions
            .extend([rooted(0, existing), rooted(1, output)]);
        attempt.authorized_paths.extend([
            BuildFilesystemAuthorizedPath {
                operand_ordinal: 0,
                access: BuildFilesystemGrantAccess::Write,
                root: BuildFilesystemRoot::Output,
                relative_path: existing.to_vec(),
            },
            BuildFilesystemAuthorizedPath {
                operand_ordinal: 1,
                access: BuildFilesystemGrantAccess::Write,
                root: BuildFilesystemRoot::Output,
                relative_path: output.to_vec(),
            },
        ]);
    }
    attempt
}

#[test]
fn recovery_rehydrates_exact_portable_and_windows_hard_links() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut summary = replay_summary();
    summary.filesystem_operation_attempts.extend([
        hard_link_attempt(19, b"locked.bin", b"portable.bin"),
        hard_link_attempt(27, b"portable.bin", b"windows.bin"),
    ]);
    let captured = capture_verified_build_filesystem_replay_record(&summary, limits)
        .expect("exact hard-link replay encodes")
        .expect("verified hard-link replay retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact hard-link replay recovers");
    let rehydrated = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact hard-link replay rehydrates");

    assert_eq!(
        rehydrated
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 46, 46, 8, 19, 27]
    );
    assert_eq!(
        rehydrated
            .output_hard_links()
            .iter()
            .map(|hard_link| (
                hard_link.kind(),
                hard_link.existing_relative_path(),
                hard_link.output_relative_path(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                psi_checked_interpreter::FilesystemOutputHardLinkReplayKind::Portable,
                b"locked.bin".as_slice(),
                b"portable.bin".as_slice(),
            ),
            (
                psi_checked_interpreter::FilesystemOutputHardLinkReplayKind::Windows,
                b"portable.bin".as_slice(),
                b"windows.bin".as_slice(),
            ),
        ]
    );
}

#[test]
fn recovery_rejects_hard_link_without_prior_regular_file_name() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut summary = replay_summary();
    summary
        .filesystem_operation_attempts
        .push(hard_link_attempt(19, b"missing.bin", b"linked.bin"));
    assert!(capture_verified_build_filesystem_replay_record(&summary, limits).is_err());
}

#[test]
fn recovery_rehydrates_exact_output_lock_pair() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&replay_summary(), limits)
        .expect("exact lock replay encodes")
        .expect("verified lock replay retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact lock replay recovers");
    assert_eq!(recovered, captured);

    let rehydrated = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact lock replay rehydrates");
    let attempts = rehydrated.attempts();
    assert_eq!(
        attempts
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 1, 46, 46, 8]
    );
    assert_eq!(
        attempts[4].scalar_operands()[0].value(),
        psi_checked_interpreter::FilesystemScalarOperandValue::I32(6)
    );
    assert_eq!(
        attempts[5].scalar_operands()[0].value(),
        psi_checked_interpreter::FilesystemScalarOperandValue::I32(8)
    );
    assert_eq!(
        attempts[4].result(),
        Some(psi_checked_interpreter::FilesystemOperationResult::Scalar(
            0
        ))
    );
    assert_eq!(attempts[5].post_error(), Some(0));
}

#[test]
fn recovery_rejects_tampered_lock_scalar() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&replay_summary(), limits)
        .unwrap()
        .unwrap();
    let needle = exact_attempt_prefix(6);
    let replacement = exact_attempt_prefix(2);
    let offsets = captured
        .canonical_bytes()
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 1, "lock acquire framing is unique");
    let mut tampered = captured.canonical_bytes().to_vec();
    tampered[offsets[0]..offsets[0] + needle.len()].copy_from_slice(&replacement);
    assert!(recover_review_only_build_filesystem_replay_record(&tampered, limits).is_err());
}
