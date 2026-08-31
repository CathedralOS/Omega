use crate::{
    FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE, FilesystemGrantAccess, FilesystemGrantRefusal,
    FilesystemGrantRefusalReason, FilesystemGrantRootIdentity, FilesystemObservationProvider,
    FilesystemOperationAttempt, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand, FilesystemScalarOperandValue,
    filesystem_root_relative_path_is_canonical,
};

const CREATE_OPERATION_TAG: u16 = 1;
const ACCESS_DENIED_RESULT: i64 = -1;
const ACCESS_DENIED_ERROR: i32 = 13;

/// One exact compiler-policy denial of an attempted write through a Source
/// root. The retained path is the compiler-issued rooted coordinate observed
/// before physical provider-path lowering; no host path spelling survives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemSourceWriteRefusalReplayRecord {
    source_root: FilesystemGrantRootIdentity,
    source_relative_path: Vec<u8>,
}

impl FilesystemSourceWriteRefusalReplayRecord {
    pub fn new(
        source_root: FilesystemGrantRootIdentity,
        source_relative_path: Vec<u8>,
    ) -> Result<Self, String> {
        if !filesystem_root_relative_path_is_canonical(&source_relative_path, false) {
            return Err(
                "filesystem replay refused Source write path must be canonical and non-root"
                    .to_owned(),
            );
        }
        Ok(Self {
            source_root,
            source_relative_path,
        })
    }

    pub const fn source_root(&self) -> FilesystemGrantRootIdentity {
        self.source_root
    }

    pub fn source_relative_path(&self) -> &[u8] {
        &self.source_relative_path
    }
}

pub(crate) fn source_write_refusal_record_from_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Result<FilesystemSourceWriteRefusalReplayRecord, String> {
    let [mode] = attempt.scalar_operands.as_slice() else {
        return Err("filesystem replay refused Source write has no unique mode".to_owned());
    };
    let [rooted] = attempt.rooted_path_operand_resolutions.as_slice() else {
        return Err("filesystem replay refused Source write has no unique rooted path".to_owned());
    };
    let [refusal] = attempt.grant_refusals.as_slice() else {
        return Err(
            "filesystem replay refused Source write has no unique grant refusal".to_owned(),
        );
    };
    if attempt.operation_tag != CREATE_OPERATION_TAG
        || attempt.provider != FilesystemObservationProvider::RealScoped
        || attempt.outcome
            != Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(ACCESS_DENIED_RESULT),
                post_error: ACCESS_DENIED_ERROR,
            })
        || mode.operand_ordinal != 1
        || mode.value != FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE)
        || rooted.operand_ordinal != 0
        || refusal.operand_ordinal != 0
        || refusal.access != FilesystemGrantAccess::Write
        || refusal.reason != FilesystemGrantRefusalReason::OutsideGrantedRoots
        || !attempt.byte_operands.is_empty()
        || !attempt.path_like_operands.is_empty()
        || !attempt.returned_paths.is_empty()
        || !attempt.observed_byte_regions.is_empty()
        || !attempt.metadata_observations.is_empty()
        || !attempt.mutable_byte_operand_resolutions.is_empty()
        || !attempt.mutable_i64_operand_resolutions.is_empty()
        || !attempt.mutable_byte_operands.is_empty()
        || !attempt.mutable_i64_operands.is_empty()
        || !attempt.authorized_paths.is_empty()
        || !attempt.logical_handle_inputs.is_empty()
        || attempt.logical_handle_output.is_some()
        || !attempt.retired_logical_handles.is_empty()
    {
        return Err("filesystem replay refused Source write lanes are inconsistent".to_owned());
    }
    FilesystemSourceWriteRefusalReplayRecord::new(rooted.root, rooted.relative_path.clone())
}

pub(crate) fn source_write_refusal_attempt(
    record: FilesystemSourceWriteRefusalReplayRecord,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag: CREATE_OPERATION_TAG,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(ACCESS_DENIED_RESULT),
            post_error: ACCESS_DENIED_ERROR,
        }),
        scalar_operands: vec![FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(FILESYSTEM_REPLAY_OUTPUT_CREATE_MODE),
        }],
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: vec![FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: record.source_root,
            relative_path: record.source_relative_path,
        }],
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
        grant_refusals: vec![FilesystemGrantRefusal {
            operand_ordinal: 0,
            access: FilesystemGrantAccess::Write,
            reason: FilesystemGrantRefusalReason::OutsideGrantedRoots,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact_attempt() -> FilesystemOperationAttempt {
        source_write_refusal_attempt(
            FilesystemSourceWriteRefusalReplayRecord::new(
                FilesystemGrantRootIdentity::new(7).unwrap(),
                b"blocked.bin".to_vec(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn exact_refused_source_write_round_trips() {
        let attempt = exact_attempt();
        let record = source_write_refusal_record_from_attempt(&attempt).unwrap();
        assert_eq!(record.source_root().get(), 7);
        assert_eq!(record.source_relative_path(), b"blocked.bin");
        assert_eq!(source_write_refusal_attempt(record), attempt);
    }

    #[test]
    fn refused_source_write_is_replayed_as_source_policy_not_output_mutation() {
        let attempt = exact_attempt();
        let observations = crate::EvaluationObservations {
            filesystem_operation_schema_version: crate::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts: vec![attempt.clone()],
            build_included_sources: Vec::new(),
            build_log: Vec::new(),
        };
        let replay =
            crate::FilesystemReplay::from_source_write_refusal_observations(&observations).unwrap();
        assert_eq!(replay.attempts(), &[attempt]);
        assert!(!replay.executes_replay_attempt(0));
        assert!(!replay.has_output_attempts());
        assert!(replay.output_entries().is_empty());

        let mut logged = observations;
        logged.build_log = b"unrelated\n".to_vec();
        assert!(crate::FilesystemReplay::from_source_write_refusal_observations(&logged).is_err());
    }

    #[test]
    fn refused_source_write_rejects_independent_lane_mutations() {
        let mut mutations: Vec<Box<dyn FnOnce(&mut FilesystemOperationAttempt)>> = vec![
            Box::new(|attempt| attempt.operation_tag = 2),
            Box::new(|attempt| attempt.provider = FilesystemObservationProvider::Virtual),
            Box::new(|attempt| {
                attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(0),
                    post_error: ACCESS_DENIED_ERROR,
                })
            }),
            Box::new(|attempt| {
                attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
                    result: FilesystemOperationResult::Scalar(ACCESS_DENIED_RESULT),
                    post_error: 2,
                })
            }),
            Box::new(|attempt| attempt.scalar_operands[0].operand_ordinal = 0),
            Box::new(|attempt| {
                attempt.scalar_operands[0].value = FilesystemScalarOperandValue::I32(420)
            }),
            Box::new(|attempt| attempt.rooted_path_operand_resolutions[0].operand_ordinal = 1),
            Box::new(|attempt| attempt.rooted_path_operand_resolutions.clear()),
            Box::new(|attempt| attempt.grant_refusals[0].operand_ordinal = 1),
            Box::new(|attempt| attempt.grant_refusals[0].access = FilesystemGrantAccess::Read),
            Box::new(|attempt| {
                attempt.grant_refusals[0].reason = FilesystemGrantRefusalReason::Unresolvable
            }),
            Box::new(|attempt| attempt.grant_refusals.clear()),
            Box::new(|attempt| {
                attempt
                    .authorized_paths
                    .push(crate::FilesystemAuthorizedPath {
                        operand_ordinal: 0,
                        access: FilesystemGrantAccess::Write,
                        root: FilesystemGrantRootIdentity::new(7).unwrap(),
                        relative_path: b"blocked.bin".to_vec(),
                    })
            }),
        ];
        for mutate in mutations.drain(..) {
            let mut attempt = exact_attempt();
            mutate(&mut attempt);
            assert!(source_write_refusal_record_from_attempt(&attempt).is_err());
        }
    }

    #[test]
    fn refused_source_write_rejects_noncanonical_coordinates() {
        let root = FilesystemGrantRootIdentity::new(7).unwrap();
        assert!(FilesystemSourceWriteRefusalReplayRecord::new(root, Vec::new()).is_err());
        assert!(
            FilesystemSourceWriteRefusalReplayRecord::new(root, b"../escape".to_vec()).is_err()
        );
    }
}
