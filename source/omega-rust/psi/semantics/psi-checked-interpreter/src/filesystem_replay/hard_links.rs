use crate::{
    FilesystemAuthorizedPath, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
    FilesystemScalarOperandValue, filesystem_root_relative_path_is_canonical,
};

const PORTABLE_HARD_LINK_OPERATION_TAG: u16 = 19;
const WINDOWS_HARD_LINK_OPERATION_TAG: u16 = 27;

/// Exact host spelling used to create one hard-link Output entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOutputHardLinkReplayKind {
    /// Portable `link(existing, new_link)`, returning zero on success.
    Portable,
    /// Win32 `CreateHardLinkA(new_link, existing, NULL)`, returning one on success.
    Windows,
}

impl FilesystemOutputHardLinkReplayKind {
    const fn operation_tag(self) -> u16 {
        match self {
            Self::Portable => PORTABLE_HARD_LINK_OPERATION_TAG,
            Self::Windows => WINDOWS_HARD_LINK_OPERATION_TAG,
        }
    }

    const fn result(self) -> i64 {
        match self {
            Self::Portable => 0,
            Self::Windows => 1,
        }
    }
}

/// One exact successful additional name for a prior Output regular file.
///
/// Both names remain under one compiler-owned Output root and both require
/// write authority. Tree validation separately requires the existing name to
/// have been established earlier in authored order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputHardLinkReplayRecord {
    kind: FilesystemOutputHardLinkReplayKind,
    output_root: FilesystemGrantRootIdentity,
    existing_relative_path: Vec<u8>,
    output_relative_path: Vec<u8>,
}

impl FilesystemOutputHardLinkReplayRecord {
    pub fn new(
        kind: FilesystemOutputHardLinkReplayKind,
        output_root: FilesystemGrantRootIdentity,
        existing_relative_path: Vec<u8>,
        output_relative_path: Vec<u8>,
    ) -> Result<Self, String> {
        if !valid_output_path(&existing_relative_path) || !valid_output_path(&output_relative_path)
        {
            return Err("filesystem replay Output hard-link path is not canonical".to_owned());
        }
        if existing_relative_path == output_relative_path {
            return Err(
                "filesystem replay Output hard link requires two distinct names".to_owned(),
            );
        }
        Ok(Self {
            kind,
            output_root,
            existing_relative_path,
            output_relative_path,
        })
    }

    pub const fn kind(&self) -> FilesystemOutputHardLinkReplayKind {
        self.kind
    }

    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        self.output_root
    }

    pub fn existing_relative_path(&self) -> &[u8] {
        &self.existing_relative_path
    }

    pub fn output_relative_path(&self) -> &[u8] {
        &self.output_relative_path
    }

    pub const fn result(&self) -> i64 {
        self.kind.result()
    }

    pub const fn post_error(&self) -> i32 {
        0
    }
}

pub(crate) fn output_hard_link_record_from_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<FilesystemOutputHardLinkReplayRecord, String> {
    validate_output_hard_link_attempt(attempt)?;
    let kind = match attempt.operation_tag {
        PORTABLE_HARD_LINK_OPERATION_TAG => FilesystemOutputHardLinkReplayKind::Portable,
        WINDOWS_HARD_LINK_OPERATION_TAG => FilesystemOutputHardLinkReplayKind::Windows,
        _ => return Err(hard_link_shape_error()),
    };
    let (existing, output) = match kind {
        FilesystemOutputHardLinkReplayKind::Portable => (
            &attempt.rooted_path_operand_resolutions[0],
            &attempt.rooted_path_operand_resolutions[1],
        ),
        FilesystemOutputHardLinkReplayKind::Windows => (
            &attempt.rooted_path_operand_resolutions[1],
            &attempt.rooted_path_operand_resolutions[0],
        ),
    };
    FilesystemOutputHardLinkReplayRecord::new(
        kind,
        existing.root,
        existing.relative_path.clone(),
        output.relative_path.clone(),
    )
}

pub(crate) fn output_hard_link_attempt(
    record: FilesystemOutputHardLinkReplayRecord,
) -> FilesystemOperationAttempt {
    let existing_ordinal = match record.kind {
        FilesystemOutputHardLinkReplayKind::Portable => 0,
        FilesystemOutputHardLinkReplayKind::Windows => 1,
    };
    let output_ordinal = match record.kind {
        FilesystemOutputHardLinkReplayKind::Portable => 1,
        FilesystemOutputHardLinkReplayKind::Windows => 0,
    };
    let existing = FilesystemRootedPathOperandResolution {
        operand_ordinal: existing_ordinal,
        root: record.output_root,
        relative_path: record.existing_relative_path.clone(),
    };
    let output = FilesystemRootedPathOperandResolution {
        operand_ordinal: output_ordinal,
        root: record.output_root,
        relative_path: record.output_relative_path.clone(),
    };
    let rooted_path_operand_resolutions = match record.kind {
        FilesystemOutputHardLinkReplayKind::Portable => vec![existing.clone(), output.clone()],
        FilesystemOutputHardLinkReplayKind::Windows => vec![output.clone(), existing.clone()],
    };
    let authorized_paths = match record.kind {
        FilesystemOutputHardLinkReplayKind::Portable => {
            vec![authorized_path(existing), authorized_path(output)]
        }
        FilesystemOutputHardLinkReplayKind::Windows => {
            vec![authorized_path(existing), authorized_path(output)]
        }
    };
    FilesystemOperationAttempt {
        operation_tag: record.kind.operation_tag(),
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(record.kind.result()),
            post_error: 0,
        }),
        scalar_operands: match record.kind {
            FilesystemOutputHardLinkReplayKind::Portable => Vec::new(),
            FilesystemOutputHardLinkReplayKind::Windows => vec![FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I64(0),
            }],
        },
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions,
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths,
        logical_handle_inputs: Vec::new(),
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

fn validate_output_hard_link_attempt(attempt: &FilesystemOperationAttempt) -> Result<(), String> {
    let kind = match attempt.operation_tag {
        PORTABLE_HARD_LINK_OPERATION_TAG => FilesystemOutputHardLinkReplayKind::Portable,
        WINDOWS_HARD_LINK_OPERATION_TAG => FilesystemOutputHardLinkReplayKind::Windows,
        _ => return Err(hard_link_shape_error()),
    };
    let [first, second] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return Err(hard_link_shape_error());
    };
    let (existing, output, expected_authorized_ordinals) = match kind {
        FilesystemOutputHardLinkReplayKind::Portable => (first, second, [0, 1]),
        FilesystemOutputHardLinkReplayKind::Windows => (second, first, [1, 0]),
    };
    let [authorized_existing, authorized_output] = attempt.authorized_paths.as_slice() else {
        return Err(hard_link_shape_error());
    };
    let scalar_shape_matches = match kind {
        FilesystemOutputHardLinkReplayKind::Portable => attempt.scalar_operands.is_empty(),
        FilesystemOutputHardLinkReplayKind::Windows => matches!(
            attempt.scalar_operands.as_slice(),
            [FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I64(0),
            }]
        ),
    };
    if attempt.provider != FilesystemObservationProvider::RealScoped
        || attempt.outcome
            != Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(kind.result()),
                post_error: 0,
            })
        || first.operand_ordinal != 0
        || second.operand_ordinal != 1
        || existing.root != output.root
        || !valid_output_path(&existing.relative_path)
        || !valid_output_path(&output.relative_path)
        || existing.relative_path == output.relative_path
        || authorized_existing.operand_ordinal != expected_authorized_ordinals[0]
        || authorized_output.operand_ordinal != expected_authorized_ordinals[1]
        || !authorization_matches(authorized_existing, existing)
        || !authorization_matches(authorized_output, output)
        || !scalar_shape_matches
        || !only_hard_link_lanes(attempt)
    {
        return Err(hard_link_shape_error());
    }
    Ok(())
}

fn valid_output_path(path: &[u8]) -> bool {
    path.len() <= super::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORY_PATH_BYTES
        && filesystem_root_relative_path_is_canonical(path, false)
}

fn authorization_matches(
    authorized: &FilesystemAuthorizedPath,
    rooted: &FilesystemRootedPathOperandResolution,
) -> bool {
    authorized.access == FilesystemGrantAccess::Write
        && authorized.root == rooted.root
        && authorized.relative_path == rooted.relative_path
}

fn authorized_path(rooted: FilesystemRootedPathOperandResolution) -> FilesystemAuthorizedPath {
    FilesystemAuthorizedPath {
        operand_ordinal: rooted.operand_ordinal,
        access: FilesystemGrantAccess::Write,
        root: rooted.root,
        relative_path: rooted.relative_path,
    }
}

fn only_hard_link_lanes(attempt: &FilesystemOperationAttempt) -> bool {
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

fn hard_link_shape_error() -> String {
    "filesystem replay Output hard-link creation is internally inconsistent".to_owned()
}
