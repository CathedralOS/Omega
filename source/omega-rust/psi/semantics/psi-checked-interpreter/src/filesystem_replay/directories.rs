use crate::{
    FilesystemAuthorizedPath, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
    FilesystemScalarOperandValue, FilesystemSourceInputReplayRecord,
    filesystem_root_relative_path_is_canonical, source_input_record_attempts,
};

const CREATE_DIRECTORY_OPERATION_TAG: u16 = 11;

/// Canonical Unix directory creation mode used by `std::fs::create_dir`.
pub const FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE: i32 = 493;

/// Explicit custody ceiling for one replayed Output directory tree.
pub const MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES: usize = 4_096;

/// Explicit custody ceiling for each retained root-relative directory path.
pub const MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES: usize = 4_096;

/// Explicit aggregate custody ceiling for all retained directory paths.
pub const MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES: usize = 16 * 1024 * 1024;

/// One exact successful creation of a fresh Output directory. Nested paths are
/// valid only when every parent appears earlier in the surrounding typed
/// replay record. Final emptiness is established separately by replayed
/// namespace equality and staged-tree custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputDirectoryReplayRecord {
    output_root: FilesystemGrantRootIdentity,
    output_relative_path: Vec<u8>,
}

/// Typed record for the bounded Source-input plus one ordered empty Output
/// directory-tree replay grammar. This deliberately does not admit files,
/// trusted-name variants, failures, or additional namespace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputOutputDirectoryReplayRecord {
    source_input: FilesystemSourceInputReplayRecord,
    output_directories: Vec<FilesystemOutputDirectoryReplayRecord>,
}

impl FilesystemInputOutputDirectoryReplayRecord {
    pub fn new(
        source_input: FilesystemSourceInputReplayRecord,
        output_directories: Vec<FilesystemOutputDirectoryReplayRecord>,
    ) -> Result<Self, String> {
        validate_output_directory_records(&output_directories)?;
        let source_attempts = source_input_record_attempts(source_input.clone());
        if source_attempts_use_root(&source_attempts, output_directories[0].output_root()) {
            return Err("filesystem replay Source and Output roots must be distinct".to_owned());
        }
        Ok(Self {
            source_input,
            output_directories,
        })
    }

    pub const fn source_input(&self) -> &FilesystemSourceInputReplayRecord {
        &self.source_input
    }

    pub fn output_directories(&self) -> &[FilesystemOutputDirectoryReplayRecord] {
        &self.output_directories
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        FilesystemSourceInputReplayRecord,
        Vec<FilesystemOutputDirectoryReplayRecord>,
    ) {
        (self.source_input, self.output_directories)
    }
}

impl FilesystemOutputDirectoryReplayRecord {
    pub fn new(
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
    ) -> Result<Self, String> {
        if output_relative_path.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES {
            return Err(format!(
                "filesystem replay Output directory path exceeds its {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES}-byte ceiling"
            ));
        }
        if !filesystem_root_relative_path_is_canonical(&output_relative_path, false) {
            return Err("filesystem replay Output directory path is not canonical".to_owned());
        }
        Ok(Self {
            output_root,
            output_relative_path,
        })
    }

    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        self.output_root
    }

    pub fn output_relative_path(&self) -> &[u8] {
        &self.output_relative_path
    }

    pub const fn mode(&self) -> i32 {
        FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE
    }

    pub const fn result(&self) -> i64 {
        0
    }

    pub const fn post_error(&self) -> i32 {
        0
    }
}

pub(crate) fn output_directory_record_from_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<FilesystemOutputDirectoryReplayRecord, String> {
    validate_output_directory_attempt(attempt)?;
    let rooted = &attempt.rooted_path_operand_resolutions[0];
    FilesystemOutputDirectoryReplayRecord::new(rooted.root, rooted.relative_path.clone())
}

pub(crate) fn output_directory_records_from_attempts(
    attempts: &[FilesystemOperationAttempt],
) -> Result<Vec<FilesystemOutputDirectoryReplayRecord>, String> {
    let directories = attempts
        .iter()
        .map(output_directory_record_from_attempt)
        .collect::<Result<Vec<_>, _>>()?;
    validate_output_directory_records(&directories)?;
    Ok(directories)
}

pub(crate) fn validate_output_directory_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<(), String> {
    let [mode] = attempt.scalar_operands.as_slice() else {
        return Err(directory_shape_error());
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return Err(directory_shape_error());
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return Err(directory_shape_error());
    };
    if attempt.operation_tag != CREATE_DIRECTORY_OPERATION_TAG
        || attempt.provider != FilesystemObservationProvider::RealScoped
        || attempt.outcome
            != Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(0),
                post_error: 0,
            })
        || mode.operand_ordinal != 1
        || mode.value != FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE)
        || rooted.operand_ordinal != 0
        || rooted.relative_path.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
        || !filesystem_root_relative_path_is_canonical(&rooted.relative_path, false)
        || authorized.operand_ordinal != 0
        || authorized.access != FilesystemGrantAccess::Write
        || authorized.root != rooted.root
        || authorized.relative_path != rooted.relative_path
        || !only_directory_lanes(attempt)
    {
        return Err(directory_shape_error());
    }
    Ok(())
}

pub(crate) fn validate_output_directory_records(
    directories: &[FilesystemOutputDirectoryReplayRecord],
) -> Result<(), String> {
    if directories.is_empty() {
        return Err("filesystem replay Output directory tree must not be empty".to_owned());
    }
    if directories.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
        return Err(format!(
            "filesystem replay Output directory tree exceeds its {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES}-entry ceiling"
        ));
    }
    let output_root = directories[0].output_root;
    let mut retained_path_bytes = 0usize;
    for (index, directory) in directories.iter().enumerate() {
        if directory.output_root != output_root {
            return Err(
                "filesystem replay Output directory tree must use one exact root".to_owned(),
            );
        }
        retained_path_bytes = retained_path_bytes
            .checked_add(directory.output_relative_path.len())
            .filter(|bytes| {
                *bytes <= MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES
            })
            .ok_or_else(|| {
                format!(
                    "filesystem replay Output directory paths exceed their {MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_RETAINED_PATH_BYTES}-byte aggregate ceiling"
                )
            })?;
        if directories[..index]
            .iter()
            .any(|prior| prior.output_relative_path == directory.output_relative_path)
        {
            return Err("filesystem replay Output directory paths must be distinct".to_owned());
        }
        if let Some(separator) = directory
            .output_relative_path
            .iter()
            .rposition(|byte| *byte == b'/')
        {
            let parent = &directory.output_relative_path[..separator];
            if !directories[..index]
                .iter()
                .any(|prior| prior.output_relative_path.as_slice() == parent)
            {
                return Err(
                    "filesystem replay nested Output directory must follow its exact parent"
                        .to_owned(),
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn output_directory_attempt(
    record: FilesystemOutputDirectoryReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: CREATE_DIRECTORY_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 0,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_MODE),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
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
            operand_ordinal: 0,
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

fn only_directory_lanes(attempt: &FilesystemOperationAttempt) -> bool {
    attempt.byte_operands.is_empty()
        && attempt.path_like_operands.is_empty()
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

pub(crate) fn source_attempts_use_root(
    attempts: &[FilesystemOperationAttempt],
    output_root: FilesystemGrantRootIdentity,
) -> bool {
    attempts.iter().any(|attempt| {
        attempt
            .rooted_path_operand_resolutions
            .iter()
            .any(|path| path.root == output_root)
            || attempt
                .authorized_paths
                .iter()
                .any(|path| path.root == output_root)
    })
}

fn directory_shape_error() -> String {
    "filesystem replay Output directory creation is internally inconsistent".to_owned()
}
