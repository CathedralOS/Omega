use crate::FilesystemSourceInputReplayRecord;
use crate::{
    FilesystemAuthorizedPath, FilesystemGrantAccess, FilesystemGrantRootIdentity,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemRootedPathOperandResolution,
    filesystem_root_relative_path_is_canonical,
};

const REMOVE_FILE_OPERATION_TAG: u16 = 9;
const REMOVE_DIRECTORY_OPERATION_TAG: u16 = 12;
const NOT_FOUND_RESULT: i64 = -1;
const NOT_FOUND_ERROR: i32 = 2;
pub const MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES: usize = 4_096;
const MAX_RETAINED_PATH_BYTES: usize = 16 * 1024 * 1024;

/// Optional Source-input prefix followed only by exact absent Output removes.
///
/// Keeping this failure-only rung distinct from the Output-tree grammar avoids
/// pretending that a no-effect failed operation is a tree entry. Mixed
/// mutation/failure lifecycles require their own later ordering validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputOutputAbsentRemovesReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    absent_removes: Vec<FilesystemOutputAbsentRemoveReplayRecord>,
}

impl FilesystemInputOutputAbsentRemovesReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        absent_removes: Vec<FilesystemOutputAbsentRemoveReplayRecord>,
    ) -> Result<Self, String> {
        if absent_removes.is_empty() {
            return Err("filesystem replay requires at least one absent Output remove".to_owned());
        }
        if absent_removes.len() > MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES {
            return Err(format!(
                "filesystem replay absent Output removes exceed the {MAX_FILESYSTEM_REPLAY_OUTPUT_ABSENT_REMOVES}-attempt ceiling"
            ));
        }
        let output_root = absent_removes[0].output_root;
        let retained_path_bytes = absent_removes.iter().try_fold(0usize, |total, remove| {
            if remove.output_root != output_root {
                return None;
            }
            total
                .checked_add(remove.output_relative_path.len())
                .filter(|total| *total <= MAX_RETAINED_PATH_BYTES)
        });
        if retained_path_bytes.is_none() {
            return Err(
                "filesystem replay absent Output removes change root or exceed the retained-path ceiling"
                    .to_owned(),
            );
        }
        Ok(Self {
            source_input,
            absent_removes,
        })
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        Vec<FilesystemOutputAbsentRemoveReplayRecord>,
    ) {
        (self.source_input, self.absent_removes)
    }
}

/// The exact removal operation attempted against an absent Output path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOutputAbsentRemoveKind {
    File,
    Directory,
}

impl FilesystemOutputAbsentRemoveKind {
    const fn operation_tag(self) -> u16 {
        match self {
            Self::File => REMOVE_FILE_OPERATION_TAG,
            Self::Directory => REMOVE_DIRECTORY_OPERATION_TAG,
        }
    }

    const fn from_operation_tag(operation_tag: u16) -> Option<Self> {
        match operation_tag {
            REMOVE_FILE_OPERATION_TAG => Some(Self::File),
            REMOVE_DIRECTORY_OPERATION_TAG => Some(Self::Directory),
            _ => None,
        }
    }
}

/// One exact authorized attempt to remove a nonexistent Output path.
///
/// The path is retained as a compiler-rooted coordinate. Replay executes the
/// operation against the fresh virtual Output namespace and therefore checks
/// both that the path is still absent and that the operation has no namespace
/// effect. Broader failure classes remain outside this first failure lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemOutputAbsentRemoveReplayRecord {
    kind: FilesystemOutputAbsentRemoveKind,
    output_root: FilesystemGrantRootIdentity,
    output_relative_path: Vec<u8>,
}

impl FilesystemOutputAbsentRemoveReplayRecord {
    pub fn new(
        kind: FilesystemOutputAbsentRemoveKind,
        output_root: FilesystemGrantRootIdentity,
        output_relative_path: Vec<u8>,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&output_relative_path, false) {
            return Err(
                "filesystem replay absent Output remove path must be canonical and non-root"
                    .to_owned(),
            );
        }
        Ok(Self {
            kind,
            output_root,
            output_relative_path,
        })
    }

    pub const fn kind(&self) -> FilesystemOutputAbsentRemoveKind {
        self.kind
    }

    pub const fn output_root(&self) -> FilesystemGrantRootIdentity {
        self.output_root
    }

    pub fn output_relative_path(&self) -> &[u8] {
        &self.output_relative_path
    }

    pub const fn result(&self) -> i64 {
        NOT_FOUND_RESULT
    }

    pub const fn post_error(&self) -> i32 {
        NOT_FOUND_ERROR
    }
}

pub(crate) fn output_absent_remove_record_from_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<FilesystemOutputAbsentRemoveReplayRecord, String> {
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return Err("filesystem replay absent Output remove has no unique rooted path".to_owned());
    };
    let [authorized] = attempt.authorized_paths.as_slice() else {
        return Err(
            "filesystem replay absent Output remove has no unique write authorization".to_owned(),
        );
    };
    let Some(kind) = FilesystemOutputAbsentRemoveKind::from_operation_tag(attempt.operation_tag)
    else {
        return Err(
            "filesystem replay absent Output remove has an unsupported operation".to_owned(),
        );
    };
    if attempt.provider != FilesystemObservationProvider::RealScoped
        || attempt.outcome
            != Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(NOT_FOUND_RESULT),
                post_error: NOT_FOUND_ERROR,
            })
        || rooted.operand_ordinal != 0
        || authorized.operand_ordinal != 0
        || authorized.access != FilesystemGrantAccess::Write
        || authorized.root != rooted.root
        || authorized.relative_path != rooted.relative_path
        || !attempt.scalar_operands.is_empty()
        || !attempt.byte_operands.is_empty()
        || !attempt.path_like_operands.is_empty()
        || !attempt.returned_paths.is_empty()
        || !attempt.observed_byte_regions.is_empty()
        || !attempt.metadata_observations.is_empty()
        || !attempt.mutable_byte_operand_resolutions.is_empty()
        || !attempt.mutable_i64_operand_resolutions.is_empty()
        || !attempt.mutable_byte_operands.is_empty()
        || !attempt.mutable_i64_operands.is_empty()
        || !attempt.logical_handle_inputs.is_empty()
        || attempt.logical_handle_output.is_some()
        || !attempt.retired_logical_handles.is_empty()
        || !attempt.grant_refusals.is_empty()
    {
        return Err("filesystem replay absent Output remove lanes are inconsistent".to_owned());
    }
    FilesystemOutputAbsentRemoveReplayRecord::new(kind, rooted.root, rooted.relative_path.clone())
}

pub(crate) fn output_absent_remove_attempt(
    record: FilesystemOutputAbsentRemoveReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: record.kind.operation_tag(),
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(NOT_FOUND_RESULT),
            post_error: NOT_FOUND_ERROR,
        }),
        scalar_operands: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> FilesystemGrantRootIdentity {
        FilesystemGrantRootIdentity::new(9).unwrap()
    }

    #[test]
    fn absent_remove_round_trips_exact_rooted_failure() {
        let record = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::Directory,
            root(),
            b"generated/missing.bin".to_vec(),
        )
        .unwrap();
        let attempt = output_absent_remove_attempt(record.clone());
        assert_eq!(
            output_absent_remove_record_from_attempt(&attempt),
            Ok(record)
        );
    }

    #[test]
    fn absent_remove_rejects_noncanonical_paths_and_changed_failure_lanes() {
        assert!(
            FilesystemOutputAbsentRemoveReplayRecord::new(
                FilesystemOutputAbsentRemoveKind::File,
                root(),
                Vec::new(),
            )
            .is_err()
        );
        assert!(
            FilesystemOutputAbsentRemoveReplayRecord::new(
                FilesystemOutputAbsentRemoveKind::File,
                root(),
                b"../escape".to_vec(),
            )
            .is_err()
        );

        let record = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::File,
            root(),
            b"missing.bin".to_vec(),
        )
        .unwrap();
        let mut changed = output_absent_remove_attempt(record);
        changed.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 13,
        });
        assert!(output_absent_remove_record_from_attempt(&changed).is_err());
    }

    #[test]
    fn failure_only_record_requires_one_bounded_root() {
        let first = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::File,
            root(),
            b"first.bin".to_vec(),
        )
        .unwrap();
        let second = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::Directory,
            root(),
            b"second".to_vec(),
        )
        .unwrap();
        assert!(
            FilesystemInputOutputAbsentRemovesReplayRecord::new(None, vec![first.clone(), second])
                .is_ok()
        );
        assert!(FilesystemInputOutputAbsentRemovesReplayRecord::new(None, Vec::new()).is_err());

        let other = FilesystemOutputAbsentRemoveReplayRecord::new(
            FilesystemOutputAbsentRemoveKind::File,
            FilesystemGrantRootIdentity::new(10).unwrap(),
            b"third.bin".to_vec(),
        )
        .unwrap();
        assert!(
            FilesystemInputOutputAbsentRemovesReplayRecord::new(None, vec![first, other]).is_err()
        );
    }
}
