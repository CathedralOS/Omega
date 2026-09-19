//! Reconstructing source input records, metadata, read chains and
//! descriptor attempts from the retained source operation attempts.

use crate::filesystem_replay::native_query_chains::source_native_handle_query_chain_attempts;
use crate::filesystem_replay::source_directories::source_directory_chain_attempts;
use crate::filesystem_replay::source_read_links::source_read_link_attempt;
use crate::filesystem_replay::{
    FilesystemReplayReadKind, FilesystemSourceDescriptorMetadataReplayRecord,
    FilesystemSourceInputReplayEventRecord, FilesystemSourceInputReplayRecord,
    FilesystemSourcePathMetadataReplayRecord, FilesystemSourceReadChainReplayRecord,
};
use crate::{
    FILESYSTEM_METADATA_API_CARRIER_BYTES, FilesystemAuthorizedPath, FilesystemGrantAccess,
    FilesystemGrantRootIdentity, FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemMetadataObservationKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemObservationProvider,
    FilesystemObservedByteRegion, FilesystemObservedByteRegionKind, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand, FilesystemScalarOperandValue,
    filesystem_root_relative_path_is_canonical,
};

pub(crate) fn source_input_record_attempts(
    record: FilesystemSourceInputReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let attempt_count = record.events.iter().fold(0usize, |count, event| {
        count
            + match event {
                FilesystemSourceInputReplayEventRecord::ReadChain(chain) => chain.reads.len() + 2,
                FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => chain
                    .attempt_count()
                    .expect("typed directory replay count fits"),
                FilesystemSourceInputReplayEventRecord::ReadLink(_) => 1,
                FilesystemSourceInputReplayEventRecord::DescriptorMetadata(_) => 3,
                FilesystemSourceInputReplayEventRecord::PathMetadata(_) => 1,
                FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain) => {
                    chain.attempt_count()
                }
            }
    });
    let mut attempts = Vec::with_capacity(attempt_count);
    for event in record.events {
        match event {
            FilesystemSourceInputReplayEventRecord::ReadChain(chain) => {
                attempts.extend(source_read_chain_attempts(chain));
            }
            FilesystemSourceInputReplayEventRecord::DirectoryReadChain(chain) => {
                attempts.extend(source_directory_chain_attempts(chain));
            }
            FilesystemSourceInputReplayEventRecord::ReadLink(read_link) => {
                attempts.push(source_read_link_attempt(read_link));
            }
            FilesystemSourceInputReplayEventRecord::DescriptorMetadata(metadata) => {
                attempts.extend(source_descriptor_metadata_attempts(metadata));
            }
            FilesystemSourceInputReplayEventRecord::PathMetadata(metadata) => {
                attempts.push(source_path_metadata_attempt(metadata));
            }
            FilesystemSourceInputReplayEventRecord::NativeHandleQueryChain(chain) => {
                attempts.extend(source_native_handle_query_chain_attempts(chain));
            }
        }
    }
    attempts
}

pub(crate) fn source_attempts_overlap_output<T: std::borrow::Borrow<FilesystemOperationAttempt>>(
    attempts: &[T],
    output_root: FilesystemGrantRootIdentity,
    output_identity: FilesystemLogicalHandleIdentity,
) -> bool {
    attempts.iter().any(|attempt| {
        let attempt = attempt.borrow();
        attempt
            .rooted_path_operand_resolutions
            .iter()
            .any(|path| path.root == output_root)
            || attempt
                .authorized_paths
                .iter()
                .any(|path| path.root == output_root)
            || attempt.logical_handle_output.is_some_and(|output| {
                output.identity == output_identity
                    || matches!(
                        output.source,
                        FilesystemLogicalHandleOutputSource::Duplicated(source)
                            | FilesystemLogicalHandleOutputSource::Borrowed(source)
                            if source == output_identity
                    )
            })
            || attempt.logical_handle_inputs.iter().any(|input| {
                input.resolution
                    == FilesystemLogicalHandleInputResolution::Resolved(output_identity)
            })
            || attempt.retired_logical_handles.contains(&output_identity)
            || attempt.result() == Some(FilesystemOperationResult::LogicalHandle(output_identity))
    })
}

pub(crate) fn source_path_metadata_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    let expected_kind = match attempt.operation_tag {
        38 => FilesystemMetadataObservationKind::FollowedPath,
        40 => FilesystemMetadataObservationKind::UnfollowedFinalPath,
        _ => return false,
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return false;
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return false;
    };
    let [metadata] = attempt.metadata_observations.as_slice() else {
        return false;
    };
    let [mutable_resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return false;
    };
    let [mutable] = attempt.mutable_byte_operands.as_slice() else {
        return false;
    };
    attempt.provider == FilesystemObservationProvider::RealScoped
        && attempt.result() == Some(FilesystemOperationResult::Scalar(0))
        && attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
        && rooted.operand_ordinal == 0
        && filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        && attempt.returned_paths.is_empty()
        && attempt.observed_byte_regions.is_empty()
        && metadata.output_operand_ordinal == 1
        && metadata.kind == expected_kind
        && mutable_resolution.operand_ordinal == 1
        && mutable.operand_ordinal == 1
        && mutable_resolution.bytes == mutable.pre_bytes
        && mutable.pre_bytes.len() == mutable.post_bytes.len()
        && mutable.post_bytes.len() >= FILESYSTEM_METADATA_API_CARRIER_BYTES
        && authorized.operand_ordinal == 0
        && authorized.access == FilesystemGrantAccess::Read
        && authorized.root == rooted.root
        && filesystem_root_relative_path_is_canonical(&authorized.relative_path, true)
        && attempt.mutable_i64_operand_resolutions.is_empty()
        && attempt.mutable_i64_operands.is_empty()
        && attempt.logical_handle_inputs.is_empty()
        && attempt.logical_handle_output.is_none()
        && attempt.retired_logical_handles.is_empty()
        && attempt.grant_refusals.is_empty()
}

pub(crate) fn source_path_metadata_attempt(
    record: FilesystemSourcePathMetadataReplayRecord,
) -> FilesystemOperationAttempt {
    let operation_tag = match record.kind {
        FilesystemMetadataObservationKind::FollowedPath => 38,
        FilesystemMetadataObservationKind::UnfollowedFinalPath => 40,
        FilesystemMetadataObservationKind::OpenDescriptor => {
            unreachable!("validated source path metadata cannot target a descriptor")
        }
    };
    FilesystemOperationAttempt {
        operation_tag,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.source_root,
            relative_path: record.source_relative_path,
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: vec![record.metadata],
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.mutable_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.mutable_pre_state,
            post_bytes: record.mutable_post_state,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Read,
            root: record.authorized_root,
            relative_path: record.authorized_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

pub(crate) fn source_read_chain_attempts(
    record: FilesystemSourceReadChainReplayRecord,
) -> Vec<FilesystemOperationAttempt> {
    let identity = record.logical_handle_identity;
    let open = source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let read_count = record.reads.len();
    let reads = record.reads.into_iter().map(|read| {
        let read_length =
            usize::try_from(read.read_result).expect("validated replay read result fits usize");
        let (operation_tag, scalar_operands, region_kind) = match read.read_kind {
            FilesystemReplayReadKind::Positioned { offset } => (
                6,
                vec![
                    FilesystemScalarOperand {
                        operand_ordinal: 2,
                        value: FilesystemScalarOperandValue::U64(read.requested_count),
                    },
                    FilesystemScalarOperand {
                        operand_ordinal: 3,
                        value: FilesystemScalarOperandValue::I64(offset),
                    },
                ],
                FilesystemObservedByteRegionKind::PositionedFileRead,
            ),
            FilesystemReplayReadKind::Sequential => (
                4,
                vec![FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(read.requested_count),
                }],
                FilesystemObservedByteRegionKind::SequentialFileRead,
            ),
        };
        FilesystemOperationAttempt {
            operation_tag,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(read.read_result),
                post_error: read.read_post_error,
            }),
            scalar_operands,
            byte_operands: Vec::new(),
            path_like_operands: Vec::new(),
            rooted_path_operand_resolutions: Vec::new(),
            returned_paths: Vec::new(),
            observed_byte_regions: vec![FilesystemObservedByteRegion {
                output_operand_ordinal: 1,
                kind: region_kind,
                offset: 0,
                length: read_length,
            }],
            metadata_observations: Vec::new(),
            mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
                operand_ordinal: 1,
                bytes: read.mutable_resolution,
            }],
            mutable_i64_operand_resolutions: Vec::new(),
            mutable_byte_operands: vec![FilesystemMutableByteOperand {
                operand_ordinal: 1,
                pre_bytes: read.mutable_pre_state,
                post_bytes: read.mutable_post_state,
            }],
            mutable_i64_operands: Vec::new(),
            authorized_paths: Vec::new(),
            logical_handle_inputs: vec![FilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
            }],
            logical_handle_output: None,
            retired_logical_handles: Vec::new(),
            grant_refusals: Vec::new(),
        }
    });
    let close = source_descriptor_close_attempt(identity, record.close_post_error);
    let mut attempts = Vec::with_capacity(read_count + 2);
    attempts.push(open);
    attempts.extend(reads);
    attempts.push(close);
    attempts
}

pub(crate) fn source_descriptor_metadata_attempts(
    record: FilesystemSourceDescriptorMetadataReplayRecord,
) -> [FilesystemOperationAttempt; 3] {
    let identity = record.logical_handle_identity;
    let open = source_descriptor_open_attempt(
        record.source_root,
        record.source_relative_path,
        identity,
        record.open_post_error,
    );
    let metadata = FilesystemOperationAttempt {
        operation_tag: 39,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: record.metadata_post_error,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: vec![record.metadata],
        mutable_byte_operand_resolutions: vec![FilesystemMutableByteOperandResolution {
            operand_ordinal: 1,
            bytes: record.mutable_resolution,
        }],
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: vec![FilesystemMutableByteOperand {
            operand_ordinal: 1,
            pre_bytes: record.mutable_pre_state,
            post_bytes: record.mutable_post_state,
        }],
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    };
    let close = source_descriptor_close_attempt(identity, record.close_post_error);
    [open, metadata, close]
}

pub(crate) fn source_descriptor_open_attempt(
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: 2,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(identity),
            post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: source_root,
            relative_path: source_relative_path.clone(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Read,
            root: source_root,
            relative_path: source_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity,
            source: FilesystemLogicalHandleOutputSource::Created,
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

pub(crate) fn source_descriptor_close_attempt(
    identity: FilesystemLogicalHandleIdentity,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: 8,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error,
        }),
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
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Resolved(identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![identity],
        grant_refusals: Vec::new(),
    }
}
