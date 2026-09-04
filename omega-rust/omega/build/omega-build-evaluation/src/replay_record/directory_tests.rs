use super::*;
use crate::{
    BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemAuthorizedPath,
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

fn source_attempts() -> Vec<BuildFilesystemOperationAttempt> {
    let source_identity = identity(1);
    let descriptor_input = || BuildFilesystemLogicalHandleInput {
        operand_ordinal: 0,
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        resolution: BuildFilesystemLogicalHandleInputResolution::Resolved(source_identity),
    };
    let mut open = attempt(
        2,
        BuildFilesystemOperationResult::LogicalHandle(source_identity),
    );
    open.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 1,
        value: BuildFilesystemScalarOperandValue::I32(0),
    });
    open.rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Source,
            relative_path: b"main.omg".to_vec(),
        });
    open.authorized_paths.push(BuildFilesystemAuthorizedPath {
        operand_ordinal: 0,
        access: BuildFilesystemGrantAccess::Read,
        root: BuildFilesystemRoot::Source,
        relative_path: b"main.omg".to_vec(),
    });
    open.logical_handle_output = Some(BuildFilesystemLogicalHandleOutput {
        kind: BuildFilesystemLogicalHandleKind::Descriptor,
        identity: source_identity,
        source: BuildFilesystemLogicalHandleOutputSource::Created,
    });

    let mut read = attempt(4, BuildFilesystemOperationResult::Scalar(0));
    read.scalar_operands.push(BuildFilesystemScalarOperand {
        operand_ordinal: 2,
        value: BuildFilesystemScalarOperandValue::U64(0),
    });
    read.mutable_byte_operand_resolutions
        .push(BuildFilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: Vec::new(),
        });
    read.mutable_byte_operands
        .push(BuildFilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: Vec::new(),
            post_bytes: Vec::new(),
        });
    read.observed_byte_regions
        .push(BuildFilesystemObservedByteRegion {
            output_operand_ordinal: 1,
            kind: BuildFilesystemObservedByteRegionKind::SequentialFileRead,
            offset: 0,
            length: 0,
        });
    read.logical_handle_inputs.push(descriptor_input());

    let mut close = attempt(8, BuildFilesystemOperationResult::Scalar(0));
    close.logical_handle_inputs.push(descriptor_input());
    close.retired_logical_handles.push(source_identity);
    vec![open, read, close]
}

fn directory_attempt(path: &[u8]) -> BuildFilesystemOperationAttempt {
    let mut directory = attempt(11, BuildFilesystemOperationResult::Scalar(0));
    directory
        .scalar_operands
        .push(BuildFilesystemScalarOperand {
            operand_ordinal: 1,
            value: BuildFilesystemScalarOperandValue::I32(
                psi_checked_interpreter::FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE,
            ),
        });
    directory
        .rooted_path_operand_resolutions
        .push(BuildFilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: BuildFilesystemRoot::Output,
            relative_path: path.to_vec(),
        });
    directory
        .authorized_paths
        .push(BuildFilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: BuildFilesystemGrantAccess::Write,
            root: BuildFilesystemRoot::Output,
            relative_path: path.to_vec(),
        });
    directory
}

fn replay_summary() -> BuildObservationSummary {
    let mut attempts = source_attempts();
    attempts.extend([
        directory_attempt(b"generated"),
        directory_attempt(b"generated/nested"),
        directory_attempt(b"sibling"),
    ]);
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

#[test]
fn recovery_rehydrates_exact_empty_output_directory_tree() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let captured = capture_verified_build_filesystem_replay_record(&replay_summary(), limits)
        .expect("exact directory replay encodes")
        .expect("verified directory replay retains custody");
    let recovered =
        recover_review_only_build_filesystem_replay_record(captured.canonical_bytes(), limits)
            .expect("exact directory replay recovers");
    assert_eq!(recovered, captured);

    let rehydrated = rehydrate_review_only_build_filesystem_replay_record(&recovered, limits)
        .expect("exact directory replay rehydrates");
    assert_eq!(
        rehydrated
            .attempts()
            .iter()
            .map(|attempt| attempt.operation_tag())
            .collect::<Vec<_>>(),
        vec![2, 4, 8, 11, 11, 11]
    );
    let directories = rehydrated.output_directories();
    assert_eq!(
        directories
            .iter()
            .map(psi_checked_interpreter::FilesystemOutputDirectoryReplayRecord::output_relative_path)
            .collect::<Vec<_>>(),
        vec![b"generated".as_slice(), b"generated/nested", b"sibling"]
    );
    assert!(directories.iter().all(|directory| directory.mode() == 493));
}

#[test]
fn recovery_rejects_alternate_directory_shapes() {
    let limits = BuildFilesystemReplayRecordLimits::default();
    let mut variants = Vec::new();

    let mut provider = replay_summary();
    provider.filesystem_operation_attempts[3].provider = BuildFilesystemProvider::Virtual;
    variants.push(provider);

    let mut service = replay_summary();
    service.filesystem_operation_attempts[3].operation_tag = 13;
    variants.push(service);

    let mut mode = replay_summary();
    mode.filesystem_operation_attempts[3].scalar_operands[0].value =
        BuildFilesystemScalarOperandValue::I32(511);
    variants.push(mode);

    let mut result = replay_summary();
    result.filesystem_operation_attempts[3].result = BuildFilesystemOperationResult::Scalar(-1);
    variants.push(result);

    let mut post_error = replay_summary();
    post_error.filesystem_operation_attempts[3].post_error = 17;
    variants.push(post_error);

    let mut missing_parent = replay_summary();
    missing_parent.filesystem_operation_attempts.remove(3);
    variants.push(missing_parent);

    let mut late_parent = replay_summary();
    late_parent.filesystem_operation_attempts.swap(3, 4);
    variants.push(late_parent);

    let mut authorization = replay_summary();
    authorization.filesystem_operation_attempts[3].authorized_paths[0].relative_path =
        b"different".to_vec();
    variants.push(authorization);

    let mut access = replay_summary();
    access.filesystem_operation_attempts[3].authorized_paths[0].access =
        BuildFilesystemGrantAccess::Read;
    variants.push(access);

    let mut duplicate = replay_summary();
    duplicate
        .filesystem_operation_attempts
        .push(directory_attempt(b"sibling"));
    variants.push(duplicate);

    for variant in variants {
        assert!(capture_verified_build_filesystem_replay_record(&variant, limits).is_err());
    }
}

#[test]
fn recovery_rejects_directory_path_over_explicit_ceiling() {
    let mut summary = replay_summary();
    let overlong =
        vec![b'a'; psi_checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES + 1];
    summary.filesystem_operation_attempts[3].rooted_path_operand_resolutions[0].relative_path =
        overlong.clone();
    summary.filesystem_operation_attempts[3].authorized_paths[0].relative_path = overlong;
    assert!(
        capture_verified_build_filesystem_replay_record(
            &summary,
            BuildFilesystemReplayRecordLimits::default()
        )
        .is_err()
    );
}
