use super::super::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemScalarOperand, FilesystemScalarOperandValue,
};

const CHANGE_FILE_OWNER_OPERATION_TAG: u16 = 49;

/// One exact `change_file_owner` attempt through the original resolved Output
/// descriptor. Provider-free replay executes the call and compares this exact
/// outcome; the record does not infer success or failure from the arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOutputChangeFileOwnerReplayRecord {
    uid: i32,
    gid: i32,
    result: i64,
    post_error: i32,
}

impl FilesystemOutputChangeFileOwnerReplayRecord {
    pub fn new(uid: i32, gid: i32, result: i64, post_error: i32) -> Result<Self, String> {
        if !matches!(result, -1 | 0) {
            return Err(
                "filesystem replay Output change_file_owner result must be exact zero or minus one"
                    .to_owned(),
            );
        }
        Ok(Self {
            uid,
            gid,
            result,
            post_error,
        })
    }

    pub const fn uid(self) -> i32 {
        self.uid
    }

    pub const fn gid(self) -> i32 {
        self.gid
    }

    pub const fn result(self) -> i64 {
        self.result
    }

    pub const fn post_error(self) -> i32 {
        self.post_error
    }
}

pub(crate) fn output_change_file_owner_record_from_attempt(
    attempt: &FilesystemOperationAttempt,
    descriptor_identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputChangeFileOwnerReplayRecord, String> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(uid),
        },
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::I32(gid),
        },
    ] = attempt.scalar_operands.as_slice()
    else {
        return Err(
            "filesystem replay Output change_file_owner has no exact uid and gid".to_owned(),
        );
    };
    let [input] = attempt.logical_handle_inputs.as_slice() else {
        return Err(
            "filesystem replay Output change_file_owner has no unique descriptor".to_owned(),
        );
    };
    let Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(result),
        post_error,
    }) = attempt.outcome
    else {
        return Err(
            "filesystem replay Output change_file_owner has no exact returned outcome".to_owned(),
        );
    };
    if attempt.operation_tag != CHANGE_FILE_OWNER_OPERATION_TAG
        || attempt.provider != FilesystemObservationProvider::RealScoped
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(descriptor_identity)
        || !operation_has_only_change_file_owner_lanes(attempt)
    {
        return Err("filesystem replay Output change_file_owner lanes are inconsistent".to_owned());
    }
    FilesystemOutputChangeFileOwnerReplayRecord::new(*uid, *gid, result, post_error)
}

fn operation_has_only_change_file_owner_lanes(operation: &FilesystemOperationAttempt) -> bool {
    operation.byte_operands.is_empty()
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
        && operation.logical_handle_output.is_none()
        && operation.retired_logical_handles.is_empty()
        && operation.grant_refusals.is_empty()
}

pub(crate) fn output_change_file_owner_attempt(
    descriptor_identity: FilesystemLogicalHandleIdentity,
    ownership: FilesystemOutputChangeFileOwnerReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: CHANGE_FILE_OWNER_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(ownership.result),
            post_error: ownership.post_error,
        }),
        scalar_operands: vec![
            FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I32(ownership.uid),
            },
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I32(ownership.gid),
            },
        ],
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
            resolution: FilesystemLogicalHandleInputResolution::Resolved(descriptor_identity),
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EvaluationObservations, FilesystemAuthorizedPath, FilesystemGrantAccess,
        FilesystemGrantRootIdentity, FilesystemInputOutputTreeReplayRecord,
        FilesystemOutputFileOperationReplayRecord, FilesystemOutputFileReplayRecord,
        FilesystemOutputTreeEntryReplayRecord, FilesystemReplay,
    };

    fn identity(value: u64) -> FilesystemLogicalHandleIdentity {
        FilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
    }

    #[test]
    fn typed_ownership_change_retains_success_and_failure_outcomes_exactly() {
        let success = FilesystemOutputChangeFileOwnerReplayRecord::new(-1, -1, 0, 0).unwrap();
        let failure = FilesystemOutputChangeFileOwnerReplayRecord::new(0, 0, -1, 1).unwrap();
        assert_eq!(
            (
                success.uid(),
                success.gid(),
                success.result(),
                success.post_error()
            ),
            (-1, -1, 0, 0)
        );
        assert_eq!(
            (
                failure.uid(),
                failure.gid(),
                failure.result(),
                failure.post_error()
            ),
            (0, 0, -1, 1)
        );
        assert!(FilesystemOutputChangeFileOwnerReplayRecord::new(0, 0, 1, 0).is_err());
    }

    #[test]
    fn ownership_change_attempt_round_trips_exact_descriptor_and_lanes() {
        let descriptor = identity(7);
        for record in [
            FilesystemOutputChangeFileOwnerReplayRecord::new(-1, -1, 0, 0).unwrap(),
            FilesystemOutputChangeFileOwnerReplayRecord::new(0, 0, -1, 1).unwrap(),
        ] {
            let attempt = output_change_file_owner_attempt(descriptor, record);
            assert_eq!(
                output_change_file_owner_record_from_attempt(&attempt, descriptor),
                Ok(record)
            );

            let mut changed = attempt.clone();
            changed.scalar_operands[1].operand_ordinal = 1;
            assert!(output_change_file_owner_record_from_attempt(&changed, descriptor).is_err());

            let mut changed = attempt.clone();
            changed.logical_handle_inputs[0].resolution =
                FilesystemLogicalHandleInputResolution::Resolved(identity(8));
            assert!(output_change_file_owner_record_from_attempt(&changed, descriptor).is_err());

            let mut changed = attempt;
            changed.authorized_paths.push(FilesystemAuthorizedPath {
                operand_ordinal: 0,
                access: FilesystemGrantAccess::Write,
                root: FilesystemGrantRootIdentity::new(2).unwrap(),
                relative_path: b"artifact".to_vec(),
            });
            assert!(output_change_file_owner_record_from_attempt(&changed, descriptor).is_err());
        }
    }

    fn sequenced_replay() -> FilesystemReplay {
        let operations = vec![
            FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                FilesystemOutputChangeFileOwnerReplayRecord::new(-1, -1, 0, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                FilesystemOutputChangeFileOwnerReplayRecord::new(0, 0, -1, 1).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                FilesystemOutputChangeFileOwnerReplayRecord::new(-1, -1, 0, 1).unwrap(),
            ),
        ];
        let output = FilesystemOutputFileReplayRecord::with_operations(
            FilesystemGrantRootIdentity::new(2).unwrap(),
            b"owned.bin".to_vec(),
            7,
            0,
            operations,
            1,
        )
        .unwrap();
        let record = FilesystemInputOutputTreeReplayRecord::output_only(
            vec![FilesystemOutputTreeEntryReplayRecord::File(output)],
            Vec::new(),
        )
        .unwrap();
        FilesystemReplay::from_input_output_tree_record(record).unwrap()
    }

    #[test]
    fn ownership_changes_keep_exact_failure_and_error_state_sequence() {
        let replay = sequenced_replay();
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(FilesystemOperationAttempt::operation_tag)
                .collect::<Vec<_>>(),
            vec![1, 49, 49, 49, 8]
        );
        assert!((0..5).all(|index| replay.executes_replay_attempt(index)));

        let observations = EvaluationObservations::from_filesystem_operation_attempts(
            replay.attempts().to_vec(),
            Vec::new(),
        );
        let decoded = FilesystemReplay::from_input_output_observations(&observations).unwrap();
        let operations = decoded.output_files();
        let operations = operations[0].operations();
        assert!(matches!(
            operations,
            [
                FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(success),
                FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(failure),
                FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(carried),
            ] if success.result() == 0
                && success.post_error() == 0
                && failure.result() == -1
                && failure.post_error() == 1
                && carried.result() == 0
                && carried.post_error() == 1
        ));
    }

    #[test]
    fn ownership_change_parser_rejects_outcome_lineage_and_order_drift() {
        let replay = sequenced_replay();

        let mut changed = replay.attempts().to_vec();
        changed[2].outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(1),
            post_error: 1,
        });
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(changed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut changed = replay.attempts().to_vec();
        changed[2].logical_handle_inputs[0].resolution =
            FilesystemLogicalHandleInputResolution::Resolved(identity(8));
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(changed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());

        let mut changed = replay.attempts().to_vec();
        changed.swap(3, 4);
        let observations =
            EvaluationObservations::from_filesystem_operation_attempts(changed, Vec::new());
        assert!(FilesystemReplay::from_input_output_observations(&observations).is_err());
    }
}
