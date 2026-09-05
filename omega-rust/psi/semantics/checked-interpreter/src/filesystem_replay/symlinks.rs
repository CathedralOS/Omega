use crate::{
    FilesystemAuthorizedPath, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemPathLikeOperand, FilesystemRootedPathOperandResolution,
    filesystem_root_relative_path_is_canonical,
};

const SYMLINK_OPERATION_TAG: u16 = 20;

/// Explicit custody ceiling for one retained symlink target spelling.
pub const MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES: usize = 4_096;

/// One exact successful creation of a fresh Output symlink.
///
/// The target spelling is retained verbatim as path-like input. Whether that
/// spelling is a canonical self-contained staged-output target is decided by
/// staged-tree custody; this record only preserves and replays the operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputSymlinkReplayRecord {
    output_root: FilesystemGrantRootIdentity,
    output_relative_path: Vec<u8>,
    target_spelling: Vec<u8>,
}

impl FilesystemOutputSymlinkReplayRecord {
    pub fn new(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
        target_spelling: Vec<u8>,
    ) -> Result<Self, String> {
        if output_relative_path.len() > super::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
            || !filesystem_root_relative_path_is_canonical(&output_relative_path, false)
        {
            return Err("filesystem replay Output symlink path is not canonical".to_owned());
        }
        if target_spelling.is_empty()
            || target_spelling.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES
            || target_spelling.contains(&0)
        {
            return Err("filesystem replay Output symlink target spelling is invalid".to_owned());
        }
        Ok(Self {
            output_root,
            output_relative_path,
            target_spelling,
        })
    }

    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        self.output_root
    }

    pub fn output_relative_path(&self) -> &[u8] {
        &self.output_relative_path
    }

    pub fn target_spelling(&self) -> &[u8] {
        &self.target_spelling
    }

    pub const fn result(&self) -> i64 {
        0
    }

    pub const fn post_error(&self) -> i32 {
        0
    }
}

pub(crate) fn output_symlink_record_from_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<FilesystemOutputSymlinkReplayRecord, String> {
    validate_output_symlink_attempt(attempt)?;
    let rooted = &attempt.rooted_path_operand_resolutions[0];
    FilesystemOutputSymlinkReplayRecord::new(
        rooted.root,
        rooted.relative_path.clone(),
        attempt.path_like_operands[0].bytes.clone(),
    )
}

pub(crate) fn validate_output_symlink_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<(), String> {
    let [target] = attempt.path_like_operands.as_slice() else {
        return Err(symlink_shape_error());
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return Err(symlink_shape_error());
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return Err(symlink_shape_error());
    };
    if attempt.operation_tag != SYMLINK_OPERATION_TAG
        || attempt.provider != FilesystemObservationProvider::RealScoped
        || attempt.outcome
            != Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(0),
                post_error: 0,
            })
        || target.operand_ordinal != 0
        || target.bytes.is_empty()
        || target.bytes.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_SYMLINK_TARGET_BYTES
        || target.bytes.contains(&0)
        || rooted.operand_ordinal != 1
        || rooted.relative_path.len() > super::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
        || !filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        || authorized.operand_ordinal != 1
        || authorized.access != FilesystemGrantAccess::Write
        || authorized.root != rooted.root
        || authorized.relative_path != rooted.relative_path
        || !only_symlink_lanes(attempt)
    {
        return Err(symlink_shape_error());
    }
    Ok(())
}

pub(crate) fn output_symlink_attempt(
    record: FilesystemOutputSymlinkReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: SYMLINK_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 0,
        }),
        scalar_operands: Vec::new(),
        byte_operands: Vec::new(),
        path_like_operands: vec![FilesystemPathLikeOperand {
            operand_ordinal: 0,
            bytes: record.target_spelling,
        }],
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 1,
            root: record.output_root,
            relative_path: record.output_relative_path.clone(),
        }],
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: vec![FilesystemAuthorizedPath {
            operand_ordinal: 1,
            access: FilesystemGrantAccess::Write,
            root: record.output_root,
            relative_path: record.output_relative_path,
        }],
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn only_symlink_lanes(attempt: &FilesystemOperationAttempt) -> bool {
    attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.returned_paths.is_empty()
        && attempt.observed_byte_regions.is_empty()
        && attempt.metadata_observations.is_empty()
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_i64_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && attempt.mutable_i64_operands.is_empty()
        && attempt.logical_handle_inputs.is_empty()
        && attempt.logical_handle_output.is_none()
        && attempt.retired_logical_handles.is_empty()
        && attempt.grant_refusals.is_empty()
}

fn symlink_shape_error() -> String {
    "filesystem replay Output symlink creation is internally inconsistent".to_owned()
}
