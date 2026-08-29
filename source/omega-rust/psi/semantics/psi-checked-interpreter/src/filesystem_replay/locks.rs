use super::super::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInput,
    FilesystemLogicalHandleInputResolution, FilesystemLogicalHandleKind,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, FilesystemOutputFileOperationReplayRecord,
    FilesystemOutputFileReplayRecord, FilesystemScalarOperand, FilesystemScalarOperandValue,
};

const LOCK_FILE_OPERATION_TAG: u16 = 46;
const NON_BLOCKING_EXCLUSIVE_LOCK: i32 = 6;
const UNLOCK: i32 = 8;

/// A compiler sponsorship ceiling for exact successful lock/unlock pairs in
/// one replay. This is not an Omega language limit.
pub const MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS: usize = 1_024;

/// One exact successful non-blocking exclusive lock followed immediately by
/// its exact successful unlock on the original Output descriptor.
///
/// The apparently fixed fields remain explicit so the typed replay record
/// retains the authored scalars, provider results, and post-error state rather
/// than inferring them from the final Output tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemOutputLockReplayRecord {
    acquire_operation: i32,
    acquire_result: i64,
    acquire_post_error: i32,
    release_operation: i32,
    release_result: i64,
    release_post_error: i32,
}

impl FilesystemOutputLockReplayRecord {
    pub fn new(
        acquire_operation: i32,
        acquire_result: i64,
        acquire_post_error: i32,
        release_operation: i32,
        release_result: i64,
        release_post_error: i32,
    ) -> Result<Self, String> {
        if acquire_operation != NON_BLOCKING_EXCLUSIVE_LOCK
            || release_operation != UNLOCK
            || acquire_result != 0
            || release_result != 0
            || acquire_post_error != 0
            || release_post_error != 0
        {
            return Err(
                "filesystem replay Output lock must be exact successful LOCK_EX|LOCK_NB followed by LOCK_UN"
                    .to_owned(),
            );
        }
        Ok(Self {
            acquire_operation,
            acquire_result,
            acquire_post_error,
            release_operation,
            release_result,
            release_post_error,
        })
    }

    pub const fn acquire_operation(self) -> i32 {
        self.acquire_operation
    }

    pub const fn acquire_result(self) -> i64 {
        self.acquire_result
    }

    pub const fn acquire_post_error(self) -> i32 {
        self.acquire_post_error
    }

    pub const fn release_operation(self) -> i32 {
        self.release_operation
    }

    pub const fn release_result(self) -> i64 {
        self.release_result
    }

    pub const fn release_post_error(self) -> i32 {
        self.release_post_error
    }
}

pub(crate) fn validate_output_lock_replay(
    outputs: &[FilesystemOutputFileReplayRecord],
) -> Result<(), String> {
    let lock_pair_count = outputs
        .iter()
        .flat_map(|output| output.operations.iter())
        .filter(|operation| {
            matches!(
                operation,
                FilesystemOutputFileOperationReplayRecord::LockAndUnlock(_)
            )
        })
        .count();
    if lock_pair_count > MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS {
        return Err(format!(
            "filesystem replay Output locks exceed the {MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS}-pair ceiling"
        ));
    }
    Ok(())
}

pub(crate) fn output_lock_record_from_attempts(
    acquire: &FilesystemOperationAttempt,
    release: &FilesystemOperationAttempt,
    descriptor_identity: FilesystemLogicalHandleIdentity,
) -> Result<FilesystemOutputLockReplayRecord, String> {
    validate_lock_attempt(
        acquire,
        descriptor_identity,
        NON_BLOCKING_EXCLUSIVE_LOCK,
        "acquire",
    )?;
    validate_lock_attempt(release, descriptor_identity, UNLOCK, "release")?;
    FilesystemOutputLockReplayRecord::new(NON_BLOCKING_EXCLUSIVE_LOCK, 0, 0, UNLOCK, 0, 0)
}

fn validate_lock_attempt(
    attempt: &FilesystemOperationAttempt,
    descriptor_identity: FilesystemLogicalHandleIdentity,
    expected_operation: i32,
    phase: &str,
) -> Result<(), String> {
    let [input] = attempt.logical_handle_inputs.as_slice() else {
        return Err(format!(
            "filesystem replay Output lock {phase} has no unique descriptor"
        ));
    };
    let [scalar] = attempt.scalar_operands.as_slice() else {
        return Err(format!(
            "filesystem replay Output lock {phase} has no unique operation scalar"
        ));
    };
    if attempt.operation_tag != LOCK_FILE_OPERATION_TAG
        || attempt.provider != FilesystemObservationProvider::RealScoped
        || attempt.result() != Some(FilesystemOperationResult::Scalar(0))
        || attempt.post_error() != Some(0)
        || input.operand_ordinal != 0
        || input.kind != FilesystemLogicalHandleKind::Descriptor
        || input.resolution != FilesystemLogicalHandleInputResolution::Resolved(descriptor_identity)
        || scalar.operand_ordinal != 1
        || scalar.value != FilesystemScalarOperandValue::I32(expected_operation)
        || !operation_has_only_lock_lanes(attempt)
    {
        return Err(format!(
            "filesystem replay Output lock {phase} lanes are inconsistent"
        ));
    }
    Ok(())
}

fn operation_has_only_lock_lanes(operation: &FilesystemOperationAttempt) -> bool {
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

pub(crate) fn output_lock_attempts(
    descriptor_identity: FilesystemLogicalHandleIdentity,
    lock: FilesystemOutputLockReplayRecord,
) -> [FilesystemOperationAttempt; 2] {
    [
        lock_attempt(
            descriptor_identity,
            lock.acquire_operation,
            lock.acquire_result,
            lock.acquire_post_error,
        ),
        lock_attempt(
            descriptor_identity,
            lock.release_operation,
            lock.release_result,
            lock.release_post_error,
        ),
    ]
}

fn lock_attempt(
    descriptor_identity: FilesystemLogicalHandleIdentity,
    operation: i32,
    result: i64,
    post_error: i32,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: LOCK_FILE_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(result),
            post_error,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(operation),
        }],
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
    use crate::FilesystemGrantRootIdentity;

    fn identity(value: u64) -> FilesystemLogicalHandleIdentity {
        FilesystemLogicalHandleIdentity::new(value).expect("test identity is nonzero")
    }

    #[test]
    fn typed_lock_pair_retains_exact_scalars_results_and_error_state() {
        let lock = FilesystemOutputLockReplayRecord::new(6, 0, 0, 8, 0, 0)
            .expect("canonical lock pair is accepted");
        assert_eq!(lock.acquire_operation(), 6);
        assert_eq!(lock.acquire_result(), 0);
        assert_eq!(lock.acquire_post_error(), 0);
        assert_eq!(lock.release_operation(), 8);
        assert_eq!(lock.release_result(), 0);
        assert_eq!(lock.release_post_error(), 0);

        for changed in [
            (2, 0, 0, 8, 0, 0),
            (6, -1, 35, 8, 0, 0),
            (6, 0, 0, 9, 0, 0),
            (6, 0, 0, 8, -1, 9),
        ] {
            assert!(
                FilesystemOutputLockReplayRecord::new(
                    changed.0, changed.1, changed.2, changed.3, changed.4, changed.5,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn typed_lock_pair_requires_original_descriptor_lineage_and_exact_order() {
        let original = identity(7);
        let lock = FilesystemOutputLockReplayRecord::new(6, 0, 0, 8, 0, 0).unwrap();
        let [acquire, release] = output_lock_attempts(original, lock);
        assert_eq!(
            output_lock_record_from_attempts(&acquire, &release, original),
            Ok(lock)
        );
        assert!(output_lock_record_from_attempts(&release, &acquire, original).is_err());
        assert!(output_lock_record_from_attempts(&acquire, &release, identity(8)).is_err());

        let mut changed_result = acquire.clone();
        changed_result.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(-1),
            post_error: 35,
        });
        assert!(output_lock_record_from_attempts(&changed_result, &release, original).is_err());
    }

    #[test]
    fn typed_lock_pairs_have_an_explicit_aggregate_ceiling() {
        let lock = FilesystemOutputLockReplayRecord::new(6, 0, 0, 8, 0, 0).unwrap();
        let operations = vec![
            FilesystemOutputFileOperationReplayRecord::LockAndUnlock(lock);
            MAX_FILESYSTEM_REPLAY_OUTPUT_LOCK_PAIRS + 1
        ];
        let output = FilesystemOutputFileReplayRecord::with_operations(
            FilesystemGrantRootIdentity::new(1).unwrap(),
            b"locked.bin".to_vec(),
            7,
            0,
            operations,
            0,
        )
        .unwrap();
        assert!(validate_output_lock_replay(&[output]).is_err());
    }
}
