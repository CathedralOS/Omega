use super::super::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemLogicalHandleOutput, FilesystemLogicalHandleOutputSource,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemOutputFileOperationReplayRecord,
    FilesystemOutputFileReplayRecord,
};

pub const MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES: usize = 1_024;

/// One successful descriptor duplication immediately followed by successful
/// retirement of the duplicate. This first bounded duplication lane retains
/// the fresh logical identity without allowing it to escape or widening the
/// existing Output operation grammar to arbitrary descriptor graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOutputDuplicateReplayRecord {
    logical_handle_identity: FilesystemLogicalHandleIdentity,
}

impl FilesystemOutputDuplicateReplayRecord {
    pub fn new(logical_handle_identity: u64) -> Result<Self, String> {
        let logical_handle_identity = FilesystemLogicalHandleIdentity::new(logical_handle_identity)
            .ok_or_else(|| "filesystem replay duplicate identity must be nonzero".to_owned())?;
        Ok(Self {
            logical_handle_identity,
        })
    }

    pub const fn logical_handle_identity(self) -> FilesystemLogicalHandleIdentity {
        self.logical_handle_identity
    }
}

pub(crate) fn output_logical_handle_identities(
    output: &FilesystemOutputFileReplayRecord,
) -> impl Iterator<Item = FilesystemLogicalHandleIdentity> + '_ {
    std::iter::once(output.logical_handle_identity).chain(output.operations.iter().filter_map(
        |operation| match operation {
            FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(duplicate) => {
                Some(duplicate.logical_handle_identity())
            }
            _ => None,
        },
    ))
}

pub(crate) fn validate_output_duplicate_replay(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    let duplicate_count = outputs
        .iter()
        .flat_map(|output| output.operations.iter())
        .filter(|operation| {
            matches!(
                operation,
                FilesystemOutputFileOperationReplayRecord::DuplicateAndClose(_)
            )
        })
        .count();
    if duplicate_count > MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES {
        return Err(format!(
            "filesystem replay Output duplicates exceed the {MAX_FILESYSTEM_REPLAY_OUTPUT_DUPLICATES}-descriptor ceiling"
        ));
    }
    Ok(())
}

pub(crate) fn output_duplicate_record_from_attempts(
    duplicate: &FilesystemOperationAttempt,
    close: &FilesystemOperationAttempt,
    source_identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputDuplicateReplayRecord, String> {
    let [input] = duplicate.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output duplicate has no unique source".to_owned());
    };
    let Some(output) = duplicate.logical_handle_output else {
        return Err("filesystem replay Output duplicate has no fresh identity".to_owned());
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::LogicalHandle(result),
        post_error: 0,
    }) = duplicate.outcome
    else {
        return Err("filesystem replay Output duplicate must succeed".to_owned());
    };
    if duplicate.operation_tag != 45
        || duplicate.provider != FilesystemObservationProvider::RealScoped
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(source_identity)
        || output.kind != FilesystemLogicalHandleKind::Descriptor
        || output.identity != result
        || output.source != FilesystemLogicalHandleOutputSource::Duplicated(source_identity)
        || result == source_identity
        || !operation_has_only_handle_lanes(duplicate)
    {
        return Err("filesystem replay Output duplicate lanes are inconsistent".to_owned());
    }
    validate_exact_output_close(close, result, 0)?;
    FilesystemOutputDuplicateReplayRecord::new(result.get())
}

fn operation_has_only_handle_lanes(operation: &FilesystemOperationAttempt) -> bool {
    operation.scalar_operands.is_empty()
        && operation.byte_operands.is_empty()
        && operation.path_like_operands.is_empty()
        && operation.rooted_path_operand_resolutions.is_empty()
        && operation.returned_paths.is_empty()
        && operation.observed_byte_regions.is_empty()
        && operation.metadata_observations.is_empty()
        && operation.mutable_byte_operand_resolutions.is_empty()
        && operation.mutable_i64_operand_resolutions.is_empty()
        && operation.mutable_byte_operands.is_empty()
        && operation.mutable_i64_operands.is_empty()
        && operation.authorized_paths.is_empty()
        && operation.retired_logical_handles.is_empty()
        && operation.grant_refusals.is_empty()
}

fn close_has_only_handle_and_retirement_lanes(operation: &FilesystemOperationAttempt) -> bool {
    operation.scalar_operands.is_empty()
        && operation.byte_operands.is_empty()
        && operation.path_like_operands.is_empty()
        && operation.rooted_path_operand_resolutions.is_empty()
        && operation.returned_paths.is_empty()
        && operation.observed_byte_regions.is_empty()
        && operation.metadata_observations.is_empty()
        && operation.mutable_byte_operand_resolutions.is_empty()
        && operation.mutable_i64_operand_resolutions.is_empty()
        && operation.mutable_byte_operands.is_empty()
        && operation.mutable_i64_operands.is_empty()
        && operation.authorized_paths.is_empty()
        && operation.grant_refusals.is_empty()
}

fn validate_exact_output_close(
    close: &FilesystemOperationAttempt,
    identity: FilesystemLogicalHandleIdentity,
    expected_post_error: i32,
) -> Result<(), String> {
    let [input] = close.logical_handle_inputs.as_slice() else {
        return Err("filesystem replay Output close has no unique descriptor".to_owned());
    };
    let [retired] = close.retired_logical_handles.as_slice() else {
        return Err("filesystem replay Output close has no unique retirement".to_owned());
    };
    if close.operation_tag != 8
        || close.provider != FilesystemObservationProvider::RealScoped
        || close.result() != Some(FilesystemOperationResult::Scalar(0))
        || close.post_error() != Some(expected_post_error)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(identity)
        || *retired != identity
        || !close_has_only_handle_and_retirement_lanes(close)
        || close.logical_handle_output.is_some()
    {
        return Err("filesystem replay Output close lanes are inconsistent".to_owned());
    }
    Ok(())
}

pub(crate) fn output_duplicate_attempts(
    source_identity: FilesystemLogicalHandleIdentity,
    duplicate: FilesystemOutputDuplicateReplayRecord,
) -> [FilesystemOperationAttempt; 2] {
    let duplicate_identity = duplicate.logical_handle_identity();
    let duplicate_attempt = FilesystemOperationAttempt {
        operation_tag: 45,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::LogicalHandle(duplicate_identity),
            post_error: 0,
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
            resolution: FilesystemLogicalHandleInputResolution::Resolved(source_identity),
        }],
        logical_handle_output: Some(FilesystemLogicalHandleOutput {
            kind: FilesystemLogicalHandleKind::Descriptor,
            identity: duplicate_identity,
            source: FilesystemLogicalHandleOutputSource::Duplicated(source_identity),
        }),
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    };
    let close_attempt = FilesystemOperationAttempt {
        operation_tag: 8,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 0,
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
            resolution: FilesystemLogicalHandleInputResolution::Resolved(duplicate_identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: vec![duplicate_identity],
        grant_refusals: Vec::new(),
    };
    [duplicate_attempt, close_attempt]
}
