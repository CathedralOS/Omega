use super::*;
use crate::{
    FilesystemByteOperand, FilesystemMetadataField, FilesystemMetadataObservation,
    FilesystemMetadataObservationKind, FilesystemMutableByteOperandResolution,
    FilesystemMutableI64OperandResolution, FilesystemObservedByteRegion,
    FilesystemObservedByteRegionKind, FilesystemPathLikeOperand, FilesystemReturnedPath,
    FilesystemReturnedPathCompleteness, FilesystemReturnedPathKind,
    FilesystemRootedPathOperandResolution, FilesystemScalarOperand,
};

fn checked_observation_evidence_total(current: usize, additional: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES)
}

impl<'program> Evaluator<'program> {
    /// Record one exact canonical operation in call-start order around the
    /// selected provider. The placeholder preserves nesting order if argument
    /// evaluation itself invokes another filesystem operation. A provider
    /// evaluation halt aborts the entire build, so incomplete placeholders can
    /// never enter a successful measured result.
    pub(super) fn try_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Value> {
        let attempt_index = self.filesystem_operation_attempts.len();
        let replay_expected = self.expected_filesystem_replay_attempt(attempt_index, operation)?;
        let execute_replayed_output = self
            .filesystem_replay
            .as_ref()
            .is_some_and(|replay| replay.executes_output_attempt(attempt_index));
        let provider = replay_expected.as_ref().map_or_else(
            || match self.real_fs.as_ref() {
                None => FilesystemObservationProvider::Virtual,
                Some(filesystem) if filesystem.is_scoped() => {
                    FilesystemObservationProvider::RealScoped
                }
                Some(_) => FilesystemObservationProvider::RealUnscoped,
            },
            |expected| expected.provider(),
        );
        self.filesystem_operation_attempts
            .push(FilesystemOperationAttempt::pending(
                operation.operation_tag(),
                provider,
            ));
        self.filesystem_operation_attempt_stack.push(attempt_index);
        let mut outcome = match self.prepare_filesystem_call(operation, arguments, frame) {
            Ok(PreparedFilesystemPreparation {
                call,
                rooted_path_operand_resolutions,
            }) => {
                let logical_handle_plan = call.logical_handle_plan();
                let (scalar_operands, byte_operands, path_like_operands) =
                    call.operand_observation_plan();
                match call.mutable_observation_plan().and_then(|mutable_plan| {
                    self.record_operand_observations(
                        attempt_index,
                        scalar_operands,
                        byte_operands,
                        path_like_operands,
                        &rooted_path_operand_resolutions,
                        &mutable_plan,
                    )?;
                    Ok(mutable_plan)
                }) {
                    Ok(mutable_plan) => {
                        match self
                            .validate_incremental_logical_handle_inputs(
                                attempt_index,
                                &logical_handle_plan,
                            )
                            .and_then(|()| {
                                self.reject_cross_domain_logical_handle_inputs(&logical_handle_plan)
                            }) {
                            Err(halt) => Err(halt),
                            Ok(()) => {
                                let served = match replay_expected.as_ref() {
                                    Some(expected) if execute_replayed_output => self
                                        .serve_executed_replay_filesystem_call(
                                            attempt_index,
                                            expected,
                                            call,
                                        ),
                                    Some(expected) => self.serve_replayed_filesystem_call(
                                        attempt_index,
                                        &mutable_plan,
                                        expected,
                                    ),
                                    None => self.serve_filesystem_call(call),
                                };
                                let completed = self
                                    .complete_mutable_observations(attempt_index, &mutable_plan);
                                match (served, completed) {
                                    (Ok(value), Ok(())) => Ok((value, logical_handle_plan)),
                                    (Err(halt), Ok(())) => Err(halt),
                                    (_, Err(halt)) => Err(halt),
                                }
                            }
                        }
                    }
                    Err(halt) => Err(halt),
                }
            }
            Err(halt) => Err(halt),
        };
        if let Some(message) = self.filesystem_observation_resource_halt.take() {
            // Observation custody is compiler policy, not Omega program
            // semantics. The provider has already refused host access, and the
            // resource halt prevents build code from observing or branching on
            // the evidence ceiling.
            outcome = Err(Halt::Resource(message));
        }
        let completed_index = self
            .filesystem_operation_attempt_stack
            .pop()
            .expect("filesystem operation attempt stack must balance");
        debug_assert_eq!(completed_index, attempt_index);
        match outcome {
            Ok((value, logical_handle_plan)) => {
                let Some(result) = value.as_int() else {
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
                            FilesystemEvaluationHaltKind::Trap,
                        ));
                    return Err(Halt::Trap(format!(
                        "canonical filesystem operation `{operation}` returned a non-integer value"
                    )));
                };
                if operation.result_kind() == FilesystemHostResultKind::I32
                    && i32::try_from(result).is_err()
                {
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
                            FilesystemEvaluationHaltKind::Trap,
                        ));
                    return Err(Halt::Trap(format!(
                        "canonical filesystem operation `{operation}` returned `{result}` outside its i32 result type"
                    )));
                }
                if let Err(halt) =
                    self.validate_observed_byte_regions(attempt_index, operation, result)
                {
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
                            FilesystemEvaluationHaltKind::Trap,
                        ));
                    return Err(halt);
                }
                if let Err(halt) =
                    self.validate_metadata_observations(attempt_index, operation, result)
                {
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
                            FilesystemEvaluationHaltKind::Trap,
                        ));
                    return Err(halt);
                }
                if let Err(halt) = self.complete_logical_handle_observations(
                    attempt_index,
                    &logical_handle_plan,
                    result,
                ) {
                    let kind = match &halt {
                        Halt::Exit(_) => FilesystemEvaluationHaltKind::Exit,
                        Halt::Unsupported(_) => FilesystemEvaluationHaltKind::Unsupported,
                        Halt::Trap(_) => FilesystemEvaluationHaltKind::Trap,
                        Halt::Resource(_) => FilesystemEvaluationHaltKind::ResourceExhausted,
                    };
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(kind));
                    return Err(halt);
                }
                let post_error = self
                    .real_fs
                    .as_ref()
                    .map_or(self.virtual_errno, |filesystem| filesystem.errno);
                let observation_result = self.filesystem_operation_attempts[attempt_index]
                    .logical_handle_output
                    .map_or(FilesystemOperationResult::Scalar(result), |output| {
                        FilesystemOperationResult::LogicalHandle(output.identity())
                    });
                self.filesystem_operation_attempts[attempt_index].outcome =
                    Some(FilesystemOperationAttemptOutcome::Returned {
                        result: observation_result,
                        post_error,
                    });
                if let Some(expected) = replay_expected.as_ref()
                    && &self.filesystem_operation_attempts[attempt_index] != expected
                {
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
                            FilesystemEvaluationHaltKind::Trap,
                        ));
                    return trap(format!("filesystem replay event {attempt_index} changed"));
                }
                Ok(value)
            }
            Err(halt) => {
                let kind = match &halt {
                    Halt::Exit(_) => FilesystemEvaluationHaltKind::Exit,
                    Halt::Unsupported(_) => FilesystemEvaluationHaltKind::Unsupported,
                    Halt::Trap(_) => FilesystemEvaluationHaltKind::Trap,
                    Halt::Resource(_) => FilesystemEvaluationHaltKind::ResourceExhausted,
                };
                self.filesystem_operation_attempts[attempt_index].outcome =
                    Some(FilesystemOperationAttemptOutcome::EvaluationHalted(kind));
                Err(halt)
            }
        }
    }

    pub(super) fn finish_filesystem_replay(&self) -> EvalResult<()> {
        let Some(replay) = &self.filesystem_replay else {
            return Ok(());
        };
        if self.filesystem_operation_attempts.len() != replay.attempts().len() {
            return trap(format!(
                "filesystem replay ended after {} event(s), but the record contains {}",
                self.filesystem_operation_attempts.len(),
                replay.attempts().len()
            ));
        }
        let outputs = replay.output_files();
        if outputs.is_empty() {
            if !self.build_included_sources.is_empty() {
                return trap("source-only filesystem replay observed generated-source handoff");
            }
            return Ok(());
        }
        if self.build_included_sources.as_slice() != replay.expected_included_sources() {
            return trap("filesystem replay generated-source handoff sequence changed");
        }
        let mut expected_files = std::collections::BTreeMap::new();
        for output in outputs {
            let mut expected_path = format!("/root/{}", output.output_root().get()).into_bytes();
            expected_path.push(b'/');
            expected_path.extend_from_slice(output.output_relative_path());
            let bytes = output.replayed_bytes().map_err(Halt::Resource)?;
            if expected_files.insert(expected_path, bytes).is_some() {
                return trap("filesystem replay contains duplicate Output paths");
            }
        }
        if self.virtual_files != expected_files
            || !self.virtual_fds.is_empty()
            || !self.virtual_dirs.is_empty()
            || !self.virtual_finds.is_empty()
            || !self.virtual_perms.is_empty()
            || !self.virtual_symlinks.is_empty()
            || !self.virtual_times.is_empty()
            || !self.virtual_flocks.is_empty()
        {
            return trap("filesystem replay output namespace changed");
        }
        Ok(())
    }

    fn expected_filesystem_replay_attempt(
        &self,
        attempt_index: usize,
        operation: FilesystemHostOperation,
    ) -> EvalResult<Option<FilesystemOperationAttempt>> {
        let Some(replay) = self.filesystem_replay.as_ref() else {
            return Ok(None);
        };
        let Some(expected) = replay.attempts().get(attempt_index) else {
            return trap(format!(
                "filesystem replay encountered extra event {attempt_index} ({operation})"
            ));
        };
        if expected.operation_tag() != operation.operation_tag() {
            return trap(format!(
                "filesystem replay event {attempt_index} changed order: expected tag {}, got {}",
                expected.operation_tag(),
                operation.operation_tag()
            ));
        }
        Ok(Some(expected.clone()))
    }

    fn serve_replayed_filesystem_call(
        &mut self,
        attempt_index: usize,
        mutable_plan: &PreparedFilesystemMutableObservationPlan,
        expected: &FilesystemOperationAttempt,
    ) -> EvalResult<Value> {
        let current = &self.filesystem_operation_attempts[attempt_index];
        if !replay_prepared_inputs_match(current, expected) {
            return trap(format!(
                "filesystem replay event {attempt_index} prepared inputs changed"
            ));
        }
        mutable_plan.apply_replay_post_state(
            &expected.mutable_byte_operands,
            &expected.mutable_i64_operands,
        )?;
        let current = &mut self.filesystem_operation_attempts[attempt_index];
        current.returned_paths = expected.returned_paths.clone();
        current.observed_byte_regions = expected.observed_byte_regions.clone();
        current.metadata_observations = expected.metadata_observations.clone();
        current.authorized_paths = expected.authorized_paths.clone();
        current.grant_refusals = expected.grant_refusals.clone();
        let Some(FilesystemOperationAttemptOutcome::Returned { result, post_error }) =
            expected.outcome
        else {
            return trap(format!(
                "filesystem replay event {attempt_index} has no returned outcome"
            ));
        };
        self.virtual_errno = post_error;
        let raw = match result {
            FilesystemOperationResult::Scalar(value) => value,
            FilesystemOperationResult::LogicalHandle(identity) => i64::try_from(identity.get())
                .map_err(|_| {
                    Halt::Trap(format!(
                        "filesystem replay event {attempt_index} logical handle exceeds i64"
                    ))
                })?,
        };
        Ok(Value::Int(raw))
    }

    fn serve_executed_replay_filesystem_call(
        &mut self,
        attempt_index: usize,
        expected: &FilesystemOperationAttempt,
        call: PreparedFilesystemCall,
    ) -> EvalResult<Value> {
        let current = &self.filesystem_operation_attempts[attempt_index];
        if !replay_prepared_inputs_match(current, expected) {
            return trap(format!(
                "filesystem replay event {attempt_index} prepared inputs changed"
            ));
        }
        // The fresh virtual namespace executes the operation. Root authorization
        // remains compiler-issued evidence from the already validated record;
        // the virtual provider has no ambient host root from which to derive it.
        self.filesystem_operation_attempts[attempt_index].authorized_paths =
            expected.authorized_paths.clone();
        self.serve_filesystem_call(call)
    }

    fn record_operand_observations(
        &mut self,
        attempt_index: usize,
        scalar_operands: Vec<FilesystemScalarOperand>,
        byte_operands: Vec<FilesystemByteOperand>,
        path_like_operands: Vec<FilesystemPathLikeOperand>,
        rooted_path_operand_resolutions: &[FilesystemRootedPathOperandResolution],
        mutable_plan: &PreparedFilesystemMutableObservationPlan,
    ) -> EvalResult<()> {
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        if attempt.scalar_operands != scalar_operands
            || attempt.byte_operands != byte_operands
            || attempt.path_like_operands != path_like_operands
        {
            return Err(Halt::Trap(
                "incremental filesystem operand evidence disagrees with the fully prepared call"
                    .to_owned(),
            ));
        }
        if attempt.rooted_path_operand_resolutions != rooted_path_operand_resolutions {
            return Err(Halt::Trap(
                "incremental filesystem rooted-path resolution evidence disagrees with the fully prepared call"
                    .to_owned(),
            ));
        }
        self.validate_incremental_mutable_operand_resolutions(attempt_index, mutable_plan)?;
        let retained_bytes = mutable_plan.reserved_bytes();
        let Some(next_total) = retained_bytes.and_then(|bytes| {
            checked_observation_evidence_total(self.filesystem_observation_evidence_bytes, bytes)
        }) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        let attempt = &mut self.filesystem_operation_attempts[attempt_index];
        let (mutable_byte_operands, mutable_i64_operands) = mutable_plan.initial_rows();
        attempt.mutable_byte_operands = mutable_byte_operands;
        attempt.mutable_i64_operands = mutable_i64_operands;
        Ok(())
    }

    pub(super) fn record_returned_path_observation(
        &mut self,
        operand_ordinal: u8,
        kind: FilesystemReturnedPathKind,
        completeness: FilesystemReturnedPathCompleteness,
        bytes: &[u8],
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        let attempt_index = *self
            .filesystem_operation_attempt_stack
            .last()
            .expect("returned-path observation requires an active filesystem attempt");
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .returned_paths
            .push(FilesystemReturnedPath {
                operand_ordinal,
                kind,
                completeness,
                bytes: bytes.to_owned(),
            });
        Ok(())
    }

    pub(super) fn record_observed_byte_region(
        &mut self,
        output_operand_ordinal: u8,
        kind: FilesystemObservedByteRegionKind,
        output: &PreparedByteOutput,
        offset: usize,
        length: usize,
    ) -> EvalResult<()> {
        let end = offset.checked_add(length).ok_or_else(|| {
            Halt::Trap("filesystem observed-byte region length overflowed".to_owned())
        })?;
        if end > output.capacity() {
            return Err(Halt::Trap(format!(
                "filesystem observed-byte region {offset}..{end} exceeds output capacity {}",
                output.capacity()
            )));
        }
        let attempt_index = *self
            .filesystem_operation_attempt_stack
            .last()
            .expect("observed-byte region requires an active filesystem attempt");
        self.filesystem_operation_attempts[attempt_index]
            .observed_byte_regions
            .push(FilesystemObservedByteRegion {
                output_operand_ordinal,
                kind,
                offset,
                length,
            });
        Ok(())
    }

    fn validate_incremental_mutable_operand_resolutions(
        &self,
        attempt_index: usize,
        plan: &PreparedFilesystemMutableObservationPlan,
    ) -> EvalResult<()> {
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        if plan.matches_resolution_roles(
            &attempt.mutable_byte_operand_resolutions,
            &attempt.mutable_i64_operand_resolutions,
        ) {
            Ok(())
        } else {
            Err(Halt::Trap(
                "incremental filesystem mutable-carrier resolution evidence disagrees with the fully prepared call"
                    .to_owned(),
            ))
        }
    }

    pub(super) fn record_prepared_filesystem_scalar_operand(
        &mut self,
        attempt_index: usize,
        operand: FilesystemScalarOperand,
    ) {
        self.filesystem_operation_attempts[attempt_index]
            .scalar_operands
            .push(operand);
    }

    pub(super) fn record_prepared_filesystem_byte_operand(
        &mut self,
        attempt_index: usize,
        operand: FilesystemByteOperand,
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            operand.bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .byte_operands
            .push(operand);
        Ok(())
    }

    pub(super) fn record_prepared_filesystem_path_like_operand(
        &mut self,
        attempt_index: usize,
        operand: FilesystemPathLikeOperand,
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            operand.bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .path_like_operands
            .push(operand);
        Ok(())
    }

    pub(super) fn record_prepared_filesystem_rooted_path_operand_resolution(
        &mut self,
        attempt_index: usize,
        resolution: FilesystemRootedPathOperandResolution,
    ) -> EvalResult<()> {
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            resolution.relative_path.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .rooted_path_operand_resolutions
            .push(resolution);
        Ok(())
    }

    pub(super) fn record_prepared_filesystem_mutable_byte_operand_resolution(
        &mut self,
        attempt_index: usize,
        operand_ordinal: u8,
        output: &PreparedByteOutput,
    ) -> EvalResult<()> {
        let bytes = output.snapshot()?;
        let Some(next_total) = checked_observation_evidence_total(
            self.filesystem_observation_evidence_bytes,
            bytes.len(),
        ) else {
            return Err(Halt::Resource(format!(
                "filesystem observation evidence exceeded its {MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES}-byte operand-evidence ceiling"
            )));
        };
        self.filesystem_observation_evidence_bytes = next_total;
        self.filesystem_operation_attempts[attempt_index]
            .mutable_byte_operand_resolutions
            .push(FilesystemMutableByteOperandResolution {
                operand_ordinal,
                bytes,
            });
        Ok(())
    }

    pub(super) fn record_prepared_filesystem_mutable_i64_operand_resolution(
        &mut self,
        attempt_index: usize,
        operand_ordinal: u8,
        value: i64,
    ) {
        self.filesystem_operation_attempts[attempt_index]
            .mutable_i64_operand_resolutions
            .push(FilesystemMutableI64OperandResolution {
                operand_ordinal,
                value,
            });
    }

    fn complete_mutable_observations(
        &mut self,
        attempt_index: usize,
        plan: &PreparedFilesystemMutableObservationPlan,
    ) -> EvalResult<()> {
        let (mutable_byte_operands, mutable_i64_operands) = plan.completed_rows()?;
        let attempt = &mut self.filesystem_operation_attempts[attempt_index];
        attempt.mutable_byte_operands = mutable_byte_operands;
        attempt.mutable_i64_operands = mutable_i64_operands;
        Ok(())
    }

    fn validate_observed_byte_regions(
        &self,
        attempt_index: usize,
        operation: FilesystemHostOperation,
        result: i64,
    ) -> EvalResult<()> {
        let expected = match operation {
            FilesystemHostOperation::Read if result >= 0 => usize::try_from(result)
                .ok()
                .map(|length| (FilesystemObservedByteRegionKind::SequentialFileRead, length)),
            FilesystemHostOperation::ReadAt if result >= 0 => usize::try_from(result)
                .ok()
                .map(|length| (FilesystemObservedByteRegionKind::PositionedFileRead, length)),
            FilesystemHostOperation::ReadDir if result >= 0 => usize::try_from(result)
                .ok()
                .map(|length| (FilesystemObservedByteRegionKind::DirectoryRecords, length)),
            FilesystemHostOperation::FindFirst if result >= 0 => Some((
                FilesystemObservedByteRegionKind::FindEntry,
                FIND_DATA_OUTPUT_BYTES,
            )),
            FilesystemHostOperation::FindNext if result == 1 => Some((
                FilesystemObservedByteRegionKind::FindEntry,
                FIND_DATA_OUTPUT_BYTES,
            )),
            FilesystemHostOperation::FindNext if result == 0 => {
                Some((FilesystemObservedByteRegionKind::FindEntry, 0))
            }
            _ => None,
        };
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        let Some((expected_kind, expected_length)) = expected else {
            if attempt.observed_byte_regions.is_empty() {
                return Ok(());
            }
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an unexpected observed-byte region"
            )));
        };

        let [region] = &attempt.observed_byte_regions[..] else {
            return Err(Halt::Trap(format!(
                "successful canonical filesystem operation `{operation}` did not retain exactly one observed-byte region"
            )));
        };
        if region.kind != expected_kind
            || region.output_operand_ordinal != 1
            || region.offset != 0
            || region.length != expected_length
        {
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an inconsistent observed-byte region"
            )));
        }
        let Some(output) = attempt
            .mutable_byte_operands
            .iter()
            .find(|output| output.operand_ordinal == region.output_operand_ordinal)
        else {
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an observed-byte region without its output carrier"
            )));
        };
        let end = region.offset.checked_add(region.length).ok_or_else(|| {
            Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an overflowing observed-byte region"
            ))
        })?;
        if end > output.post_bytes.len() {
            return Err(Halt::Trap(format!(
                "canonical filesystem operation `{operation}` retained an out-of-bounds observed-byte region"
            )));
        }
        Ok(())
    }

    fn validate_metadata_observations(
        &self,
        attempt_index: usize,
        operation: FilesystemHostOperation,
        result: i64,
    ) -> EvalResult<()> {
        let expected_kind = match (operation, result) {
            (FilesystemHostOperation::ReadMetadata, 0) => {
                Some(FilesystemMetadataObservationKind::FollowedPath)
            }
            (FilesystemHostOperation::ReadFileMetadata, 0) => {
                Some(FilesystemMetadataObservationKind::OpenDescriptor)
            }
            (FilesystemHostOperation::ReadSymlinkMetadata, 0) => {
                Some(FilesystemMetadataObservationKind::UnfollowedFinalPath)
            }
            _ => None,
        };
        let attempt = &self.filesystem_operation_attempts[attempt_index];
        let Some(expected_kind) = expected_kind else {
            if attempt.metadata_observations.is_empty() {
                return Ok(());
            }
            return trap(format!(
                "canonical filesystem operation `{operation}` retained an unexpected metadata observation"
            ));
        };
        let [observation] = &attempt.metadata_observations[..] else {
            return trap(format!(
                "successful canonical filesystem operation `{operation}` did not retain exactly one metadata observation"
            ));
        };
        if observation.kind() != expected_kind || observation.output_operand_ordinal() != 1 {
            return trap(format!(
                "canonical filesystem operation `{operation}` retained an inconsistent metadata observation"
            ));
        }
        let Some(output) = attempt
            .mutable_byte_operands
            .iter()
            .find(|output| output.operand_ordinal == observation.output_operand_ordinal())
        else {
            return trap(format!(
                "canonical filesystem operation `{operation}` retained metadata without its output carrier"
            ));
        };
        let expected_carrier = self.canonical_metadata_carrier(
            *observation,
            output.post_bytes.len(),
            "retained metadata",
        )?;
        if output.post_bytes != expected_carrier {
            return trap(format!(
                "canonical filesystem operation `{operation}` metadata carrier disagrees with its selected-target semantic row"
            ));
        }
        Ok(())
    }

    fn canonical_metadata_carrier(
        &self,
        observation: FilesystemMetadataObservation,
        capacity: usize,
        context: &str,
    ) -> EvalResult<Vec<u8>> {
        if capacity < STAT_OUTPUT_BYTES || self.filesystem_metadata_layout.record_size() > capacity
        {
            return trap(format!(
                "{context} requires the {STAT_OUTPUT_BYTES}-byte filesystem metadata API carrier and selected {}-byte record, but holds {capacity} bytes",
                self.filesystem_metadata_layout.record_size()
            ));
        }
        let mut carrier = vec![0u8; capacity];
        for field in FilesystemMetadataField::ALL {
            let placement = self.filesystem_metadata_layout.field_layout(field);
            let width = usize::from(placement.stored_width_bits() / 8);
            let bytes = if let Some(value) = observation.unsigned_field(field) {
                let maximum = match placement.stored_width_bits() {
                    16 => u64::from(u16::MAX),
                    32 => u64::from(u32::MAX),
                    64 => u64::MAX,
                    _ => unreachable!("validated metadata stored width"),
                };
                if value > maximum {
                    return trap(format!(
                        "filesystem metadata field {field:?} value {value} exceeds its selected {}-bit carrier",
                        placement.stored_width_bits()
                    ));
                }
                value.to_le_bytes()
            } else {
                let value = observation
                    .signed_field(field)
                    .expect("metadata field is either signed or unsigned");
                let fits = match placement.stored_width_bits() {
                    16 => i16::try_from(value).is_ok(),
                    32 => i32::try_from(value).is_ok(),
                    64 => true,
                    _ => unreachable!("validated metadata stored width"),
                };
                if !fits {
                    return trap(format!(
                        "filesystem metadata field {field:?} value {value} exceeds its selected {}-bit carrier",
                        placement.stored_width_bits()
                    ));
                }
                value.to_le_bytes()
            };
            let start = placement.offset();
            carrier[start..start + width].copy_from_slice(&bytes[..width]);
        }
        Ok(carrier)
    }

    pub(super) fn record_prepared_filesystem_logical_handle_input(
        &mut self,
        attempt_index: usize,
        operand_ordinal: u8,
        kind: FilesystemLogicalHandleKind,
        raw: i64,
        null_allowed: bool,
    ) {
        let resolution = self
            .filesystem_logical_handles
            .resolve(kind, raw, null_allowed);
        self.filesystem_operation_attempts[attempt_index]
            .logical_handle_inputs
            .push(FilesystemLogicalHandleInput {
                operand_ordinal,
                kind,
                resolution,
            });
    }

    fn validate_incremental_logical_handle_inputs(
        &mut self,
        attempt_index: usize,
        plan: &PreparedFilesystemLogicalHandlePlan,
    ) -> EvalResult<()> {
        let canonical_inputs = plan
            .inputs
            .iter()
            .map(|input| FilesystemLogicalHandleInput {
                operand_ordinal: input.operand_ordinal,
                kind: input.kind,
                resolution: self.filesystem_logical_handles.resolve(
                    input.kind,
                    input.raw,
                    input.null_allowed,
                ),
            })
            .collect::<Vec<_>>();
        if self.filesystem_operation_attempts[attempt_index].logical_handle_inputs
            != canonical_inputs
        {
            return trap(
                "incremental filesystem logical-handle evidence disagrees with the fully prepared call",
            );
        }
        Ok(())
    }

    fn reject_cross_domain_logical_handle_inputs(
        &self,
        plan: &PreparedFilesystemLogicalHandlePlan,
    ) -> EvalResult<()> {
        if plan.inputs.iter().any(|input| {
            self.filesystem_logical_handles.conflicts_with_live_domain(
                input.kind,
                input.raw,
                input.null_allowed,
            )
        }) {
            return trap(
                "filesystem input aliases a live token from another logical-handle domain",
            );
        }
        Ok(())
    }

    fn complete_logical_handle_observations(
        &mut self,
        attempt_index: usize,
        plan: &PreparedFilesystemLogicalHandlePlan,
        result: i64,
    ) -> EvalResult<()> {
        if plan
            .input_success
            .is_some_and(|success| success.accepts(result))
            && self.filesystem_operation_attempts[attempt_index]
                .logical_handle_inputs
                .iter()
                .any(|input| {
                    matches!(
                        input.resolution,
                        FilesystemLogicalHandleInputResolution::Unknown
                    )
                })
        {
            return trap(
                "filesystem provider accepted an input outside its live logical-handle domain",
            );
        }
        if let Some(output) = plan.output {
            let (success, kind, source) = match output {
                PreparedFilesystemLogicalHandleOutput::Created { kind, success } => {
                    (success, kind, FilesystemLogicalHandleOutputSource::Created)
                }
                PreparedFilesystemLogicalHandleOutput::Duplicated {
                    source_operand_ordinal,
                    success,
                } => {
                    let source = self.resolved_logical_handle_input(
                        attempt_index,
                        source_operand_ordinal,
                        FilesystemLogicalHandleKind::Descriptor,
                    );
                    if success.accepts(result) && source.is_none() {
                        return trap(
                            "filesystem provider duplicated an unresolved descriptor token",
                        );
                    }
                    (
                        success,
                        FilesystemLogicalHandleKind::Descriptor,
                        source
                            .map(FilesystemLogicalHandleOutputSource::Duplicated)
                            .unwrap_or(FilesystemLogicalHandleOutputSource::Created),
                    )
                }
                PreparedFilesystemLogicalHandleOutput::Borrowed {
                    source_operand_ordinal,
                    success,
                } => {
                    let source = self.resolved_logical_handle_input(
                        attempt_index,
                        source_operand_ordinal,
                        FilesystemLogicalHandleKind::Descriptor,
                    );
                    if success.accepts(result) && source.is_none() {
                        return trap(
                            "filesystem provider returned a native view for an unresolved descriptor token",
                        );
                    }
                    (
                        success,
                        FilesystemLogicalHandleKind::Native,
                        source
                            .map(FilesystemLogicalHandleOutputSource::Borrowed)
                            .unwrap_or(FilesystemLogicalHandleOutputSource::Created),
                    )
                }
            };
            if success.accepts(result) {
                let identity = match source {
                    FilesystemLogicalHandleOutputSource::Borrowed(source) => self
                        .filesystem_logical_handles
                        .borrow_native(result, source),
                    FilesystemLogicalHandleOutputSource::Created
                    | FilesystemLogicalHandleOutputSource::Duplicated(_) => {
                        self.filesystem_logical_handles.create(kind, result)
                    }
                }
                .map_err(filesystem_logical_handle_halt)?;
                self.filesystem_operation_attempts[attempt_index].logical_handle_output =
                    Some(FilesystemLogicalHandleOutput {
                        kind,
                        identity,
                        source,
                    });
            }
        }

        if let Some(retirement) = plan.retirement
            && retirement.success.accepts(result)
        {
            let input = self.filesystem_operation_attempts[attempt_index]
                .logical_handle_inputs
                .iter()
                .find(|input| input.operand_ordinal == retirement.operand_ordinal)
                .copied()
                .expect("retirement plan must name one retained handle input");
            let FilesystemLogicalHandleInputResolution::Resolved(identity) = input.resolution
            else {
                return trap("filesystem provider closed an unresolved logical handle token");
            };
            self.filesystem_operation_attempts[attempt_index].retired_logical_handles =
                self.filesystem_logical_handles.retire(input.kind, identity);
        }
        Ok(())
    }

    fn resolved_logical_handle_input(
        &self,
        attempt_index: usize,
        operand_ordinal: u8,
        kind: FilesystemLogicalHandleKind,
    ) -> Option<crate::FilesystemLogicalHandleIdentity> {
        self.filesystem_operation_attempts[attempt_index]
            .logical_handle_inputs
            .iter()
            .find(|input| input.operand_ordinal == operand_ordinal && input.kind == kind)
            .and_then(|input| match input.resolution {
                FilesystemLogicalHandleInputResolution::Resolved(identity) => Some(identity),
                FilesystemLogicalHandleInputResolution::Null
                | FilesystemLogicalHandleInputResolution::Unknown => None,
            })
    }

    /// Drive a value-returning `FilesystemHost` operation against the selected
    /// filesystem provider. Canonical preparation has already evaluated every
    /// authored operand exactly once and validated every mutable output place.
    fn serve_filesystem_call(&mut self, call: PreparedFilesystemCall) -> EvalResult<Value> {
        // REAL-filesystem mode (build.omg rung; opt-in via
        // `FilesystemAccess::RealUnscoped`): the whole op family routes to the
        // real provider with the same exhaustive operation set.
        if self.real_fs.is_some() {
            return self.try_real_filesystem_call(call);
        }
        // Value-returning raw `FilesystemHost` ops, matching the native seam:
        // each returns its "syscall" result (fd / byte count / rc; negative on
        // error) against the deterministic in-memory filesystem.
        let result: i64 = match call {
            PreparedFilesystemCall::Create { path, mode: _ } => {
                // O_WRONLY|O_CREAT|O_TRUNC: create/truncate, writable.
                self.virtual_open(path, true, true) as i64
            }
            PreparedFilesystemCall::Open { path, flags } => {
                self.virtual_open_flags(path, flags) as i64
            }
            PreparedFilesystemCall::OpenPathHandle {
                path,
                desired_access: _,
                share_mode: _,
                security_attributes: _,
                creation_disposition: _,
                flags_and_attributes: _,
                template_file: _,
            } => {
                // Hermetic CreateFileA model for metadata/query handles. The
                // wrapper supplies access=0 + OPEN_EXISTING; the virtual fd
                // table already models both files and read-only directories.
                let fd = self.virtual_open_flags(path, 0);
                if fd < 0 {
                    // `GetLastError`, not CRT errno, is the native error source.
                    self.virtual_errno = match self.virtual_errno {
                        13 => 5,   // EACCES -> ERROR_ACCESS_DENIED
                        9 => 6,    // EBADF -> ERROR_INVALID_HANDLE
                        17 => 183, // EEXIST -> ERROR_ALREADY_EXISTS
                        _ => 2,    // ERROR_FILE_NOT_FOUND
                    };
                }
                fd as i64
            }
            PreparedFilesystemCall::OpenCreate { path, flags, mode } => {
                // `open(path, flags, mode)` with O_CREAT (Rust `File::create_new`,
                // `OpenOptions.create`/`.create_new`). Flag bits are the HOST's
                // (host_open_flags, mirroring the checked target encoder). This
                // adds the O_EXCL/EEXIST atomic
                // create-new guard + create-mode recording; every other flag bit
                // (O_TRUNC/O_APPEND/access/EACCES/ENOENT) is handled by the shared
                // `virtual_open_flags`, so `open_create` cleanly SUBSUMES `open`.
                let exists = self.virtual_files.contains_key(&path)
                    || self.virtual_dirs.contains(&path)
                    || self.virtual_char_devices.contains(&path);
                if host_open_flags::o_creat(flags) && host_open_flags::o_excl(flags) && exists {
                    self.virtual_errno = 17; // EEXIST (O_CREAT|O_EXCL, path present)
                    -1
                } else {
                    // Whether this call actually creates the file (records the mode
                    // AFTER the open so the create's own access is not gated by it).
                    let created = host_open_flags::o_creat(flags) && !exists;
                    let fd = self.virtual_open_flags(path.clone(), flags);
                    if fd >= 0 && created {
                        self.virtual_perms.insert(path, (mode as u32) & 0o777);
                    }
                    fd as i64
                }
            }
            PreparedFilesystemCall::Read { fd, buffer, count } => {
                match self.virtual_read_n(fd, count.host) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        buffer.write(&bytes)?;
                        self.record_observed_byte_region(
                            1,
                            FilesystemObservedByteRegionKind::SequentialFileRead,
                            &buffer,
                            0,
                            bytes.len(),
                        )?;
                        n
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Write { fd, bytes } => {
                match self.virtual_write(fd, &bytes) {
                    Some(count) => count as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::ReadAt {
                fd,
                buffer,
                count,
                offset,
            } => {
                // `pread(fd, buf, count, offset)`: read at an absolute offset
                // WITHOUT moving the cursor (Rust `FileExt::read_at`).
                match self.virtual_read_at(fd, offset, count.host) {
                    Some(bytes) => {
                        let n = bytes.len() as i64;
                        buffer.write(&bytes)?;
                        self.record_observed_byte_region(
                            1,
                            FilesystemObservedByteRegionKind::PositionedFileRead,
                            &buffer,
                            0,
                            bytes.len(),
                        )?;
                        n
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::WriteAt { fd, bytes, offset } => {
                // `pwrite(fd, buf, count, offset)`: write at an absolute offset
                // WITHOUT moving the cursor (Rust `FileExt::write_at`).
                match self.virtual_write_at(fd, offset, &bytes) {
                    Some(count) => count as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Close { fd } => {
                if self.virtual_fds.remove(&fd).is_some() {
                    // Closing the owning fd releases any advisory lock it held.
                    self.virtual_flocks.retain(|_, owner| *owner != fd);
                    0
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
            PreparedFilesystemCall::CloseHandle { handle } => {
                match synthetic_handle_fd(handle) {
                    Some(handle) if self.virtual_fds.remove(&handle).is_some() => {
                        self.virtual_flocks.retain(|_, owner| *owner != handle);
                        1 // Win32 BOOL success
                    }
                    _ => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::Duplicate { fd } => {
                // `dup(fd)`: mint a fresh descriptor over the same open file (Rust
                // `File::try_clone`). Both descriptor lifetimes share the same
                // open-file-description cursor. EBADF for an unknown fd.
                let clone = self.virtual_fds.get(&fd).map(|descriptor| VirtualFd {
                    path: descriptor.path.clone(),
                    cursor: std::rc::Rc::clone(&descriptor.cursor),
                    writable: descriptor.writable,
                    is_dir: descriptor.is_dir,
                });
                match clone {
                    Some(clone) => {
                        let new_fd = self.virtual_next_fd;
                        self.virtual_next_fd += 1;
                        self.virtual_fds.insert(new_fd, clone);
                        new_fd as i64
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::LockFile { fd, operation } => {
                // `flock(fd, operation)`: advisory whole-file lock (Rust
                // `File::lock`/`lock_shared`/`try_lock`/`unlock`). operation
                // bitmask: LOCK_SH=1, LOCK_EX=2, LOCK_NB=4, LOCK_UN=8. The
                // hermetic model tracks EXCLUSIVE ownership per path; a
                // non-blocking acquire on a path another fd holds is EWOULDBLOCK.
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                    Some(path) if operation & 8 != 0 => {
                        // LOCK_UN: release this fd's lock (a no-op if it held none).
                        if self.virtual_flocks.get(&path) == Some(&fd) {
                            self.virtual_flocks.remove(&path);
                        }
                        0
                    }
                    Some(path) => {
                        let held_by_other = matches!(
                            self.virtual_flocks.get(&path),
                            Some(owner) if *owner != fd
                        );
                        if held_by_other && operation & 4 != 0 {
                            self.virtual_errno = 35; // EWOULDBLOCK (== EAGAIN)
                            -1
                        } else {
                            self.virtual_flocks.insert(path, fd);
                            0
                        }
                    }
                }
            }
            PreparedFilesystemCall::LockFileEx {
                handle,
                flags,
                reserved: _,
                length_low: _,
                length_high: _,
                overlapped: _,
            } => {
                // Win32 LockFileEx over the synthetic fd/HANDLE. flags:
                // EXCLUSIVE=2, FAIL_IMMEDIATELY=1. The range/OVERLAPPED
                // arguments are ABI-shape inputs; the std wrapper always asks
                // for offset zero and the whole file.
                let Some(fd) = synthetic_handle_fd(handle) else {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    return Ok(Value::Int(0));
                };
                let flags = flags as i32;
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                    Some(path) => {
                        let held_by_other = matches!(
                            self.virtual_flocks.get(&path),
                            Some(owner) if *owner != fd
                        );
                        if held_by_other && flags & 1 != 0 {
                            self.virtual_errno = 33; // ERROR_LOCK_VIOLATION
                            0
                        } else {
                            self.virtual_flocks.insert(path, fd);
                            1
                        }
                    }
                }
            }
            PreparedFilesystemCall::UnlockFile {
                handle,
                offset_low: _,
                offset_high: _,
                length_low: _,
                length_high: _,
            } => {
                let Some(fd) = synthetic_handle_fd(handle) else {
                    self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                    return Ok(Value::Int(0));
                };
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                    Some(path) if self.virtual_flocks.get(&path) == Some(&fd) => {
                        self.virtual_flocks.remove(&path);
                        1
                    }
                    Some(_) => {
                        self.virtual_errno = 158; // ERROR_NOT_LOCKED
                        0
                    }
                }
            }
            PreparedFilesystemCall::GetLastError => i64::from(self.virtual_errno),
            // `remove_name` is the TRUSTED plain-path twin (D-at trust class,
            // the create_dir_name precedent): the arg bytes ARE the path, so
            // both spellings share one model.
            PreparedFilesystemCall::Remove { path }
            | PreparedFilesystemCall::RemoveName { path } => {
                if self.virtual_files.remove(&path).is_some() {
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            PreparedFilesystemCall::Seek { fd, offset, whence } => {
                match self.virtual_seek(fd, offset, whence) {
                    Some(position) => position,
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::SetLen { fd, length } => {
                let rc = self.virtual_set_len(fd, length);
                if rc < 0 {
                    self.virtual_errno = 9; // EBADF
                }
                rc
            }
            PreparedFilesystemCall::SetFilePermissions { fd, mode } => {
                // `fchmod(fd, mode)`: record the mode against the fd's path so a
                // subsequent write-open sees it (mirrors path-based chmod). EBADF
                // if the descriptor is unknown.
                match self.virtual_fds.get(&fd) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        self.virtual_perms.insert(path, mode);
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::SetFileTimes { fd, times } => {
                // `futimens(fd, times)`: `times` is two packed `struct timespec`
                // (atime then mtime, {tv_sec i64, tv_nsec i64} each). Read the
                // modification seconds -- times[1].tv_sec at byte offset 16 -- and
                // record it against the fd's path so stat/fstat report it. EBADF if
                // the descriptor is unknown.
                match self.virtual_fds.get(&fd) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        let mtime = times
                            .bytes
                            .get(16..24)
                            .and_then(|s| <[u8; 8]>::try_from(s).ok())
                            .map(i64::from_le_bytes)
                            .unwrap_or(0);
                        self.virtual_times.insert(path, mtime);
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Sync { fd } | PreparedFilesystemCall::SyncData { fd } => {
                // `fsync(fd)`: flush to durable storage (`sync_data` aliases it --
                // macOS has no `fdatasync`). In the hermetic in-memory FS the bytes
                // are already "durable", so this is a no-op that only validates the
                // descriptor: 0 for a live fd, -1 (EBADF) otherwise -- matching the
                // native seam's contract.
                i64::from(self.virtual_fds.contains_key(&fd)) - 1
            }
            PreparedFilesystemCall::Errno => {
                // `read_errno()` (darwin `___error()` deref): the thread-local
                // errno set by the most recent failing op. Not cleared on
                // success (POSIX), so it is only meaningful right after a -1.
                i64::from(self.virtual_errno)
            }
            // The trusted plain-name variant shares create_dir's semantics
            // (the arg bytes ARE the path -- the scratch subslice excludes
            // the native NUL, so both engines see identical bytes).
            PreparedFilesystemCall::CreateDir { path, mode: _ }
            | PreparedFilesystemCall::CreateDirName {
                name: path,
                mode: _,
            } => {
                // -1 (EEXIST) if the dir already exists.
                if self.virtual_dirs.insert(path) {
                    0
                } else {
                    self.virtual_errno = 17; // EEXIST
                    -1
                }
            }
            PreparedFilesystemCall::RemoveDir { path }
            | PreparedFilesystemCall::RemoveDirName { path } => {
                if self.virtual_dirs.remove(&path) {
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            PreparedFilesystemCall::OpenAt { dirfd, name, flags } => {
                // `openat(dirfd, name, flags)`: open `name` relative to the open
                // directory `dirfd`. The full path (dirfd's path + "/" + name) is
                // joined HERE (the OS does it natively), so no Omega path build.
                match self.virtual_at_path(dirfd, &name) {
                    Some(full) => self.virtual_open_flags(full, flags) as i64,
                    None => {
                        self.virtual_errno = 9; // EBADF (dirfd not an open directory)
                        -1
                    }
                }
            }
            PreparedFilesystemCall::UnlinkAt { dirfd, name, flags } => {
                // `unlinkat(dirfd, name, flags)`: remove `name` relative to `dirfd`.
                // flags & AT_REMOVEDIR(0x80) removes an empty directory, else a file.
                match self.virtual_at_path(dirfd, &name) {
                    None => {
                        self.virtual_errno = 9; // EBADF
                        -1
                    }
                    Some(full) => {
                        let removed = if (flags & 128) != 0 {
                            self.virtual_dirs.remove(&full)
                        } else {
                            self.virtual_files.remove(&full).is_some()
                        };
                        if removed {
                            0
                        } else {
                            self.virtual_errno = 2; // ENOENT
                            -1
                        }
                    }
                }
            }
            PreparedFilesystemCall::SetPermissions { path, mode } => {
                // `chmod(path, mode)`: record the mode. ENOENT if the path names
                // neither a file nor a directory. `mode` is the second arg.
                if self.virtual_files.contains_key(&path) || self.virtual_dirs.contains(&path) {
                    self.virtual_perms.insert(path, mode);
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            PreparedFilesystemCall::ChangeOwner { path, uid, gid }
            | PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid } => {
                // `chown`/`lchown(path, uid, gid)`: change owner/group. ENOENT if
                // the path is absent. The hermetic model's process identity is
                // VIRTUAL_UID/GID (a normal, non-root user), so only a NO-OP change
                // is permitted: a uid/gid of -1 leaves that component alone, and
                // setting the CURRENT owner succeeds; any OTHER owner is EPERM --
                // exactly what native `chown` does when run as a normal user.
                // (`lchown` differs from `chown` only on symlinks, which the
                // hermetic FS never follows on ownership ops, so they behave
                // identically here.)
                let exists = self.virtual_files.contains_key(&path)
                    || self.virtual_dirs.contains(&path)
                    || self.virtual_symlinks.contains_key(&path);
                if !exists {
                    self.virtual_errno = 2; // ENOENT
                    -1
                } else {
                    self.virtual_chown_result(uid, gid)
                }
            }
            PreparedFilesystemCall::ChangeFileOwner { fd, uid, gid } => {
                // `fchown(fd, uid, gid)`: like `chown` by descriptor. EBADF for an
                // unknown fd; otherwise the same non-root ownership rule.
                if self.virtual_fds.contains_key(&fd) {
                    self.virtual_chown_result(uid, gid)
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
            }
            PreparedFilesystemCall::Rename { from, to } => {
                match self.virtual_files.remove(&from) {
                    Some(content) => {
                        self.virtual_files.insert(to, content);
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            PreparedFilesystemCall::HardLink { original, link } => {
                // `link(original, link)`: a second name for the same inode.
                // ENOENT if the original is absent; EEXIST if the link name is
                // taken. The hermetic FS has no inodes, so this COPIES the bytes
                // (approximate: a later write to one name won't show in the
                // other — see TASKS_FS.md). Enough to model create+readback.
                if self.virtual_files.contains_key(&link) || self.virtual_dirs.contains(&link) {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else if let Some(content) = self.virtual_files.get(&original).cloned() {
                    self.virtual_files.insert(link, content);
                    0
                } else {
                    self.virtual_errno = 2; // ENOENT
                    -1
                }
            }
            PreparedFilesystemCall::CreateHardLink {
                link,
                existing,
                security_attributes: _,
            } => {
                // `CreateHardLinkA(link, existing, security)` -- the WINDOWS
                // hard-link primitive (session slice 3): the ARG ORDER is
                // (new link, existing), REVERSED from `hard_link`, and the
                // result is BOOL (1 success / 0 failure). Same hermetic
                // copy-the-bytes model as `hard_link` above. virtual_errno is
                // also the provider's Win32 last-error slot for GetLastError.
                if self.virtual_files.contains_key(&link) || self.virtual_dirs.contains(&link) {
                    self.virtual_errno = 183; // ERROR_ALREADY_EXISTS
                    0
                } else if let Some(content) = self.virtual_files.get(&existing).cloned() {
                    self.virtual_files.insert(link, content);
                    1
                } else {
                    self.virtual_errno = 2; // ERROR_FILE_NOT_FOUND
                    0
                }
            }
            PreparedFilesystemCall::GetOsfHandle { fd } => {
                // `_get_osfhandle(fd)` -- the fd -> HANDLE bridge (session
                // slice 4a). The hermetic model's handles ARE its fds
                // (identity), so consumers key the same descriptor table;
                // -2 (msvcrt's bad-fd spelling) for an unknown fd.
                if self.virtual_fds.contains_key(&fd) {
                    i64::from(fd)
                } else {
                    -2
                }
            }
            PreparedFilesystemCall::FinalPathNameByHandle {
                handle,
                buffer,
                capacity,
                flags: _,
            } => {
                // `GetFinalPathNameByHandleA(handle, buffer, capacity, flags)`:
                // resolve an OPEN handle to its final path. The hermetic
                // model's canonical path IS the descriptor's stored key
                // (already absolute for its namespace; no drive letters or
                // \\?\ prefixes to synthesize), NUL-terminated into the
                // buffer. Win32 return contract: the length WITHOUT the NUL
                // when it fits, the REQUIRED size INCLUDING the NUL when the
                // capacity is too small, 0 for a bad handle (GetLastError
                // semantics -- no errno touched).
                let path = synthetic_handle_fd(handle)
                    .and_then(|handle| self.virtual_fds.get(&handle))
                    .map(|descriptor| descriptor.path.clone());
                match path {
                    Some(path) => {
                        if path.len() < capacity.host {
                            let mut bytes = path.clone();
                            bytes.push(0);
                            buffer.write(&bytes)?;
                            self.record_returned_path_observation(
                                1,
                                FilesystemReturnedPathKind::FinalPath,
                                FilesystemReturnedPathCompleteness::Complete,
                                &path,
                            )?;
                            path.len() as i64
                        } else {
                            (path.len() + 1) as i64
                        }
                    }
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::SetFileTime {
                handle,
                creation: _,
                last_access: _,
                last_write: write_ft,
            } => {
                // `SetFileTime(handle, creation, access_ft, write_ft)` (session
                // slice 4b): stamp the handle's path with the WRITE time from
                // its 8-byte FILETIME buffer (100ns units since 1601 -> unix
                // seconds via the calibration constants), the same
                // virtual_times store `set_file_times` uses. BOOL result;
                // 0 for a bad handle (GetLastError semantics -- no errno).
                match synthetic_handle_fd(handle).and_then(|handle| self.virtual_fds.get(&handle)) {
                    Some(descriptor) => {
                        let path = descriptor.path.clone();
                        let filetime = write_ft
                            .get(0..8)
                            .and_then(|s| <[u8; 8]>::try_from(s).ok())
                            .map(i64::from_le_bytes)
                            .unwrap_or(0);
                        let secs = filetime / 10_000_000 - 11_644_473_600;
                        self.virtual_times.insert(path, secs);
                        1
                    }
                    None => {
                        self.virtual_errno = 6; // ERROR_INVALID_HANDLE
                        0
                    }
                }
            }
            PreparedFilesystemCall::Symlink { target, link } => {
                // `symlink(target, linkpath)`: record the link -> target mapping.
                // EEXIST if the link name already names a file/dir/symlink.
                if self.virtual_files.contains_key(&link)
                    || self.virtual_dirs.contains(&link)
                    || self.virtual_symlinks.contains_key(&link)
                {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else {
                    self.virtual_symlinks.insert(link, target);
                    0
                }
            }
            PreparedFilesystemCall::ReadLink {
                path,
                buffer,
                count,
            } => {
                // `readlink(path, buf, count)`: write the target bytes into the
                // buffer (up to `count`), returning the number written. ENOENT if
                // `path` is not a symlink in the hermetic model.
                match self.virtual_symlinks.get(&path).cloned() {
                    Some(target) => {
                        let n = target.len().min(count.host);
                        buffer.write(&target[..n])?;
                        self.record_returned_path_observation(
                            1,
                            FilesystemReturnedPathKind::ReadLinkPayload,
                            if n == target.len() {
                                FilesystemReturnedPathCompleteness::Complete
                            } else {
                                FilesystemReturnedPathCompleteness::LimitReached
                            },
                            &target[..n],
                        )?;
                        n as i64
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            PreparedFilesystemCall::Canonicalize { path, buffer } => {
                // `realpath(path, buf)`: resolve `path` to its canonical absolute
                // form and write it NUL-terminated into the buffer. The hermetic FS
                // is already absolute and does not resolve `.`/`..`; it follows one
                // symlink level (matching `read_link`). Returns a non-zero success
                // flag (native returns the resolved-buffer pointer) or 0 (NULL) +
                // ENOENT when the target does not exist.
                let resolved = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
                let exists = self.virtual_files.contains_key(&resolved)
                    || self.virtual_dirs.contains(&resolved);
                if exists {
                    let mut bytes = resolved.clone();
                    bytes.push(0); // NUL-terminate like realpath's C string
                    buffer.write(&bytes)?;
                    self.record_returned_path_observation(
                        1,
                        FilesystemReturnedPathKind::CanonicalPath,
                        FilesystemReturnedPathCompleteness::Complete,
                        &resolved,
                    )?;
                    1
                } else {
                    self.virtual_errno = 2; // ENOENT
                    0
                }
            }
            PreparedFilesystemCall::ReadDir {
                fd,
                buffer,
                count,
                position,
            } => {
                // `read_dir(fd, buf, count, &position)`: pack the directory's
                // entries as Darwin `dirent` records and return the next window
                // of complete records. `position` is a synthetic byte cursor, so
                // repeated calls drain directories larger than one buffer just
                // like native `___getdirentries64`.
                let dir_path = self
                    .virtual_fds
                    .get(&fd)
                    .filter(|descriptor| descriptor.is_dir)
                    .map(|descriptor| descriptor.path.clone());
                match dir_path {
                    None => {
                        // Unknown fd -> EBADF; a live non-dir fd -> ENOTDIR.
                        self.virtual_errno = if self.virtual_fds.contains_key(&fd) {
                            20 // ENOTDIR
                        } else {
                            9 // EBADF
                        };
                        -1
                    }
                    Some(path) => {
                        let records = self.build_dirent_records(&path);
                        let start = position.initial.max(0) as usize;
                        let (chunk, next_position) =
                            dirent_record_chunk(&records, start, count.host);
                        if chunk.is_empty() {
                            self.record_observed_byte_region(
                                1,
                                FilesystemObservedByteRegionKind::DirectoryRecords,
                                &buffer,
                                0,
                                0,
                            )?;
                            0
                        } else {
                            let n = chunk.len();
                            buffer.write(chunk)?;
                            position.write(next_position as i64)?;
                            self.record_observed_byte_region(
                                1,
                                FilesystemObservedByteRegionKind::DirectoryRecords,
                                &buffer,
                                0,
                                n,
                            )?;
                            n as i64
                        }
                    }
                }
            }
            PreparedFilesystemCall::FindFirst { pattern, data } => {
                // `find_first(pattern, &data)` -- the windows dir-walk seam (fs
                // rung 3a). `pattern` is `dir/*`: the impl joins with `/`, which
                // Win32 accepts natively and which matches the hermetic FS keys
                // byte-exactly. Snapshot the directory's entries (".", "..",
                // then the immediate children -- the same set read_dir packs)
                // into a cursor keyed by a fresh handle, fill the FIRST entry's
                // find-data record, and return the handle; -1
                // (INVALID_HANDLE_VALUE, ENOENT) when the directory does not
                // exist. A real directory always yields "." first, so an open
                // enumeration always has a first entry -- exactly Win32.
                let entries = pattern
                    .strip_suffix(b"/*")
                    .filter(|dir_path| self.virtual_dirs.contains(*dir_path))
                    .map(|dir_path| self.build_find_entries(dir_path));
                match entries {
                    Some(mut entries) => {
                        let (name, is_dir) =
                            entries.pop_front().expect("dot entries are always present");
                        self.write_find_data(&data, &name, is_dir)?;
                        self.record_observed_byte_region(
                            1,
                            FilesystemObservedByteRegionKind::FindEntry,
                            &data,
                            0,
                            FIND_DATA_OUTPUT_BYTES,
                        )?;
                        let handle = self.virtual_next_find;
                        self.virtual_next_find += 1;
                        self.virtual_finds.insert(handle, entries);
                        handle
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            PreparedFilesystemCall::FindNext { handle, data } => {
                // `find_next(handle, &data)`: fill the next snapshotted entry
                // (1 = filled, 0 = end-of-enumeration or unknown handle).
                match self
                    .virtual_finds
                    .get_mut(&handle)
                    .and_then(std::collections::VecDeque::pop_front)
                {
                    Some((name, is_dir)) => {
                        self.write_find_data(&data, &name, is_dir)?;
                        self.record_observed_byte_region(
                            1,
                            FilesystemObservedByteRegionKind::FindEntry,
                            &data,
                            0,
                            FIND_DATA_OUTPUT_BYTES,
                        )?;
                        1
                    }
                    None => {
                        self.record_observed_byte_region(
                            1,
                            FilesystemObservedByteRegionKind::FindEntry,
                            &data,
                            0,
                            0,
                        )?;
                        0
                    }
                }
            }
            PreparedFilesystemCall::FindClose { handle } => {
                // `find_close(handle)`: release the cursor (BOOL, like Win32).
                if self.virtual_finds.remove(&handle).is_some() {
                    1
                } else {
                    0
                }
            }
            PreparedFilesystemCall::ReadMetadata { path, buffer } => {
                // `stat(path, buf)`: fill the buffer's st_mode (off 4, u16) and
                // st_size (off 96, i64) as the darwin kernel would. A regular
                // file is S_IFREG(0o100000)|0o644 with size = content length; a
                // directory is S_IFDIR(0o040000)|0o755 size 0. ENOENT otherwise.
                // st_mode = format bits (S_IFREG/S_IFDIR) | permission bits, so
                // a prior `set_permissions` (chmod) shows through `readonly()`.
                let chmod_perm = self
                    .virtual_perms
                    .get(&path)
                    .map(|mode| (*mode as u16) & 0o7777);
                let meta = if self.virtual_char_devices.contains(&path) {
                    // A character-special device (`/dev/null`): S_IFCHR|0o666, size 0.
                    Some((0o020_000u16 | chmod_perm.unwrap_or(0o666), 0i64))
                } else if let Some(content) = self.virtual_files.get(&path) {
                    let size = content.len() as i64;
                    Some((0o100_000u16 | chmod_perm.unwrap_or(0o644), size))
                } else if self.virtual_dirs.contains(&path) {
                    Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64))
                } else {
                    None
                };
                match meta {
                    Some((mode, size)) => {
                        // A `set_file_times` mtime shows through; otherwise the
                        // hermetic FS has no clock, so it reports a fixed modeled
                        // mtime (native `stat` returns the real time -- tests assert
                        // exact == in the interpreter and a lower bound natively).
                        let mtime = self
                            .virtual_times
                            .get(&path)
                            .copied()
                            .unwrap_or(VIRTUAL_MTIME_SECS);
                        self.write_fs_stat(
                            &buffer,
                            FilesystemMetadataObservationKind::FollowedPath,
                            u32::from(mode),
                            size,
                            mtime,
                        )?;
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
            PreparedFilesystemCall::ReadFileMetadata { fd, buffer } => {
                // `fstat(fd, buf)`: like `stat` but keyed by an OPEN descriptor. Map
                // the fd to its path, then fill the same stat record (a held `File`
                // is always a regular file here). EBADF for an unknown fd. Never
                // touches the cursor.
                let path = self
                    .virtual_fds
                    .get(&fd)
                    .map(|descriptor| descriptor.path.clone());
                let meta = path.and_then(|path| {
                    // A `set_file_times` mtime shows through; else the modeled epoch.
                    let mtime = self
                        .virtual_times
                        .get(&path)
                        .copied()
                        .unwrap_or(VIRTUAL_MTIME_SECS);
                    let chmod_perm = self
                        .virtual_perms
                        .get(&path)
                        .map(|mode| (*mode as u16) & 0o7777);
                    if let Some(content) = self.virtual_files.get(&path) {
                        Some((
                            0o100_000u16 | chmod_perm.unwrap_or(0o644),
                            content.len() as i64,
                            mtime,
                        ))
                    } else if self.virtual_dirs.contains(&path) {
                        Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64, mtime))
                    } else {
                        None
                    }
                });
                match meta {
                    Some((mode, size, mtime)) => {
                        self.write_fs_stat(
                            &buffer,
                            FilesystemMetadataObservationKind::OpenDescriptor,
                            u32::from(mode),
                            size,
                            mtime,
                        )?;
                        0
                    }
                    None => {
                        self.virtual_errno = 9; // EBADF (unknown descriptor)
                        -1
                    }
                }
            }
            PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } => {
                // `lstat(path, buf)`: like `stat`, but does NOT follow a final
                // symlink. A symlink reports S_IFLNK(0o120000)|0o777 with size =
                // the target path length (POSIX: a symlink's size is its target's
                // byte length); everything else is identical to `stat`.
                let meta = if let Some(target) = self.virtual_symlinks.get(&path) {
                    Some((0o120_000u16 | 0o777, target.len() as i64))
                } else {
                    let chmod_perm = self
                        .virtual_perms
                        .get(&path)
                        .map(|mode| (*mode as u16) & 0o7777);
                    if let Some(content) = self.virtual_files.get(&path) {
                        Some((
                            0o100_000u16 | chmod_perm.unwrap_or(0o644),
                            content.len() as i64,
                        ))
                    } else if self.virtual_dirs.contains(&path) {
                        Some((0o040_000u16 | chmod_perm.unwrap_or(0o755), 0i64))
                    } else {
                        None
                    }
                };
                match meta {
                    Some((mode, size)) => {
                        self.write_fs_stat(
                            &buffer,
                            FilesystemMetadataObservationKind::UnfollowedFinalPath,
                            u32::from(mode),
                            size,
                            VIRTUAL_MTIME_SECS,
                        )?;
                        0
                    }
                    None => {
                        self.virtual_errno = 2; // ENOENT
                        -1
                    }
                }
            }
        };
        Ok(Value::Int(result))
    }

    /// Build the packed darwin `dirent` records for a directory: `.` and `..`
    /// then each IMMEDIATE child (files in `virtual_files`, subdirs in
    /// `virtual_dirs` directly under `dir_path/`). Each record is
    /// `[d_ino(8) d_seekoff(8) d_reclen@16(u16) d_namlen@18(u16) d_type@20(u8)
    /// d_name@21(namlen) NUL pad]`, `d_reclen = round_up_8(25 + namlen)` — the
    /// exact layout `___getdirentries64` produces, so byte counts and a parser
    /// agree with native.
    /// Resolve `name` RELATIVE to the open directory `dirfd` to a full virtual
    /// path (`dirfd`'s path + "/" + name). Returns None if `dirfd` is not an open
    /// directory descriptor. The `*at` ops do their path-joining here -- in Rust,
    /// the way the OS does natively -- so the Omega layer never builds a path.
    fn virtual_at_path(&self, dirfd: i32, name: &[u8]) -> Option<Vec<u8>> {
        let dir = self
            .virtual_fds
            .get(&dirfd)
            .filter(|descriptor| descriptor.is_dir)
            .map(|descriptor| descriptor.path.clone())?;
        let mut full = dir;
        full.push(b'/');
        full.extend_from_slice(name);
        Some(full)
    }

    /// The find-enumeration twin of `build_dirent_records` (fs rung 3a): the
    /// same entry set (".", "..", then the immediate children of `dir_path`)
    /// as (name, is_dir) pairs for a `find_first` cursor snapshot.
    fn build_find_entries(&self, dir_path: &[u8]) -> std::collections::VecDeque<(Vec<u8>, bool)> {
        let mut entries: std::collections::VecDeque<(Vec<u8>, bool)> =
            std::collections::VecDeque::from([(b".".to_vec(), true), (b"..".to_vec(), true)]);
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        let immediate_child = |path: &[u8]| -> Option<Vec<u8>> {
            let rest = path.strip_prefix(prefix.as_slice())?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest.to_vec())
            }
        };
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_child(path) {
                entries.push_back((name, false));
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_child(path) {
                entries.push_back((name, true));
            }
        }
        entries
    }

    /// Fill a caller find-data buffer (`&mut [u8]`, >= 320 bytes) the way
    /// `FindFirstFileA`/`FindNextFileA` write WIN32_FIND_DATAA: file
    /// attributes u32 little-endian at byte 0 (FILE_ATTRIBUTE_DIRECTORY 0x10 /
    /// FILE_ATTRIBUTE_NORMAL 0x80) and the NUL-terminated entry name at byte
    /// 44. Other fields are left zero.
    pub(super) fn write_find_data(
        &self,
        output: &PreparedByteOutput,
        name: &[u8],
        is_dir: bool,
    ) -> EvalResult<()> {
        let mut record = vec![0u8; FIND_DATA_OUTPUT_BYTES];
        let attributes: u32 = if is_dir { 0x10 } else { 0x80 };
        record[0..4].copy_from_slice(&attributes.to_le_bytes());
        let name_len = name.len().min(259);
        record[44..44 + name_len].copy_from_slice(&name[..name_len]);
        output.write(&record)
    }

    fn build_dirent_records(&self, dir_path: &[u8]) -> Vec<u8> {
        let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
        let mut prefix = dir_path.to_vec();
        prefix.push(b'/');
        let immediate_child = |path: &[u8]| -> Option<Vec<u8>> {
            let rest = path.strip_prefix(prefix.as_slice())?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest.to_vec())
            }
        };
        for path in self.virtual_files.keys() {
            if let Some(name) = immediate_child(path) {
                entries.push((name, 8)); // DT_REG
            }
        }
        for path in &self.virtual_dirs {
            if let Some(name) = immediate_child(path) {
                entries.push((name, 4)); // DT_DIR
            }
        }
        pack_dirent_records(&entries)
    }

    /// Fill a caller stat buffer from one target-neutral semantic metadata row
    /// using the exact selected target layout supplied by orchestration. The
    /// complete API carrier is zeroed first, including Linux-arm64's 16-byte
    /// tail beyond its 128-byte record, so prior mutable state cannot leak into
    /// either Omega decoding or package evidence.
    pub(super) fn write_fs_stat(
        &mut self,
        output: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
        mode: u32,
        size: i64,
        mtime_secs: i64,
    ) -> EvalResult<()> {
        let observation = FilesystemMetadataObservation::new(1, kind, mode, size, mtime_secs);
        let carrier =
            self.canonical_metadata_carrier(observation, output.capacity(), "metadata output")?;
        output.write(&carrier)?;
        let attempt_index = *self
            .filesystem_operation_attempt_stack
            .last()
            .expect("metadata observation requires an active filesystem attempt");
        self.filesystem_operation_attempts[attempt_index]
            .metadata_observations
            .push(observation);
        Ok(())
    }

    /// Mint a fresh descriptor over `path`; `create` truncates (or creates) the
    /// file first.
    fn virtual_open(&mut self, path: Vec<u8>, writable: bool, create: bool) -> i32 {
        if create {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor: std::rc::Rc::new(std::cell::Cell::new(0)),
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// Write `bytes` at the descriptor's cursor (extending the file as needed),
    /// advancing the cursor. `None` if the fd is unknown or not writable.
    fn virtual_write(&mut self, fd: i32, bytes: &[u8]) -> Option<usize> {
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor.get();
        let content = self.virtual_files.get_mut(&path)?;
        let end = cursor + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[cursor..end].copy_from_slice(bytes);
        descriptor.cursor.set(end);
        Some(bytes.len())
    }

    /// Read up to `count` bytes from the descriptor's cursor, advancing it.
    /// `None` if the fd is unknown.
    fn virtual_read_n(&mut self, fd: i32, count: usize) -> Option<Vec<u8>> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor.get();
        let content = self.virtual_files.get(&path)?;
        let available = content.get(cursor..).unwrap_or(&[]);
        let take = available.len().min(count);
        let bytes = available[..take].to_vec();
        descriptor.cursor.set(cursor + take);
        Some(bytes)
    }

    /// Read up to `count` bytes starting at absolute `offset` WITHOUT moving the
    /// cursor (Rust `FileExt::read_at` / `pread`). `None` if the fd is unknown or
    /// the offset is negative. A read past end-of-file yields fewer (or zero) bytes.
    fn virtual_read_at(&mut self, fd: i32, offset: i64, count: usize) -> Option<Vec<u8>> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let content = self.virtual_files.get(&path)?;
        let available = content.get(offset as usize..).unwrap_or(&[]);
        let take = available.len().min(count);
        Some(available[..take].to_vec())
    }

    /// Write `bytes` at absolute `offset` (extending + zero-filling any gap) WITHOUT
    /// moving the cursor (Rust `FileExt::write_at` / `pwrite`). `None` if the fd is
    /// unknown, not writable, or the offset is negative.
    fn virtual_write_at(&mut self, fd: i32, offset: i64, bytes: &[u8]) -> Option<usize> {
        if offset < 0 {
            return None;
        }
        let descriptor = self.virtual_fds.get(&fd)?;
        if !descriptor.writable {
            return None;
        }
        let path = descriptor.path.clone();
        let start = offset as usize;
        let content = self.virtual_files.get_mut(&path)?;
        if bytes.is_empty() {
            return Some(0);
        }
        let end = start + bytes.len();
        if content.len() < end {
            content.resize(end, 0);
        }
        content[start..end].copy_from_slice(bytes);
        Some(bytes.len())
    }

    /// `open(path, flags)`: model the O_CREAT/O_TRUNC/O_APPEND/access bits.
    /// Returns a fresh fd, or -1 if the path is absent and O_CREAT is not set.
    fn virtual_open_flags(&mut self, path: Vec<u8>, flags: i32) -> i32 {
        // Follow one symlink level (the canonicalize/read_link model): native
        // open on BOTH families resolves symlinks, and the hermetic open never
        // did -- surfaced when the windows canonicalize composition made open
        // its entry point. The descriptor stores the RESOLVED path, so
        // handle-keyed consumers (final_path_name_by_handle) report the final
        // target exactly like Win32.
        let path = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
        let exists = self.virtual_files.contains_key(&path);
        let o_creat = host_open_flags::o_creat(flags);
        let o_trunc = host_open_flags::o_trunc(flags);
        let o_append = host_open_flags::o_append(flags);
        let writable = flags & 0x3 != 0; // O_WRONLY | O_RDWR (universal)
        // Opening a directory for writing is EISDIR (Rust `ErrorKind::IsADirectory`).
        // Checked before the ENOENT test so a dir path (never in `virtual_files`)
        // reports the more specific kind.
        if self.virtual_dirs.contains(&path) && writable {
            self.virtual_errno = 21; // EISDIR
            return -1;
        }
        // Permission enforcement: opening a chmod'd path fails with EACCES when
        // the needed bit is clear — the owner-write bit (0o200) for a write-open,
        // or the owner-read bit (0o400) for a read-open (Rust
        // `ErrorKind::PermissionDenied`).
        let needed_bit = if writable { 0o200 } else { 0o400 };
        if self
            .virtual_perms
            .get(&path)
            .is_some_and(|mode| mode & needed_bit == 0)
        {
            self.virtual_errno = 13; // EACCES
            return -1;
        }
        // Read-open of a DIRECTORY: POSIX allows opening a dir read-only (the
        // basis for `read_dir`). Mint a dir descriptor. Checked before the ENOENT
        // test since a dir path is never in `virtual_files`. (This also aligns
        // `exists`/`try_exists` on a dir with native, where opening a dir works.)
        if !writable && self.virtual_dirs.contains(&path) {
            let fd = self.virtual_next_fd;
            self.virtual_next_fd += 1;
            self.virtual_fds.insert(
                fd,
                VirtualFd {
                    path,
                    cursor: std::rc::Rc::new(std::cell::Cell::new(0)),
                    writable: false,
                    is_dir: true,
                },
            );
            return fd;
        }
        if !exists && !o_creat {
            self.virtual_errno = 2; // ENOENT
            return -1;
        }
        if !exists || o_trunc {
            self.virtual_files.insert(path.clone(), Vec::new());
        }
        let cursor = if o_append {
            self.virtual_files.get(&path).map_or(0, Vec::len)
        } else {
            0
        };
        let fd = self.virtual_next_fd;
        self.virtual_next_fd += 1;
        self.virtual_fds.insert(
            fd,
            VirtualFd {
                path,
                cursor: std::rc::Rc::new(std::cell::Cell::new(cursor)),
                writable,
                is_dir: false,
            },
        );
        fd
    }

    /// `lseek(fd, offset, whence)`: reposition the cursor, returning the new
    /// absolute offset. `None` on unknown fd, bad whence, or a negative result.
    fn virtual_seek(&mut self, fd: i32, offset: i64, whence: i32) -> Option<i64> {
        let descriptor = self.virtual_fds.get(&fd)?;
        let path = descriptor.path.clone();
        let cursor = descriptor.cursor.get() as i64;
        let len = self.virtual_files.get(&path).map_or(0, Vec::len) as i64;
        let new_pos = match whence {
            0 => offset,          // SEEK_SET
            1 => cursor + offset, // SEEK_CUR
            2 => len + offset,    // SEEK_END
            _ => return None,
        };
        if new_pos < 0 {
            return None;
        }
        descriptor.cursor.set(new_pos as usize);
        Some(new_pos)
    }

    /// `ftruncate(fd, length)`: resize the file backing `fd` (truncate or
    /// zero-extend). Returns 0 on success, -1 on an unknown fd/path.
    fn virtual_set_len(&mut self, fd: i32, length: i64) -> i64 {
        let Some(descriptor) = self.virtual_fds.get(&fd) else {
            return -1;
        };
        let path = descriptor.path.clone();
        let Some(content) = self.virtual_files.get_mut(&path) else {
            return -1;
        };
        content.resize(length.max(0) as usize, 0);
        0
    }

    /// The non-root `chown`/`fchown`/`lchown` rule shared by the ownership
    /// handlers: a change to the CURRENT owner -- or a uid/gid of -1, meaning
    /// "leave that component unchanged" -- is a permitted no-op (returns 0); any
    /// OTHER owner is EPERM (sets errno 1, returns -1). Mirrors what the native
    /// syscalls do for a normal (non-root) user, keeping the two engines'
    /// differential consistent.
    fn virtual_chown_result(&mut self, uid: i32, gid: i32) -> i64 {
        let effective_uid = if uid == -1 { VIRTUAL_UID as i32 } else { uid };
        let effective_gid = if gid == -1 { VIRTUAL_GID as i32 } else { gid };
        if effective_uid == VIRTUAL_UID as i32 && effective_gid == VIRTUAL_GID as i32 {
            0
        } else {
            self.virtual_errno = 1; // EPERM
            -1
        }
    }
}

fn replay_prepared_inputs_match(
    current: &FilesystemOperationAttempt,
    expected: &FilesystemOperationAttempt,
) -> bool {
    let byte_pre_matches = current.mutable_byte_operands.len()
        == expected.mutable_byte_operands.len()
        && current
            .mutable_byte_operands
            .iter()
            .zip(&expected.mutable_byte_operands)
            .all(|(actual, recorded)| {
                actual.operand_ordinal == recorded.operand_ordinal
                    && actual.pre_bytes == recorded.pre_bytes
            });
    let scalar_pre_matches = current.mutable_i64_operands.len()
        == expected.mutable_i64_operands.len()
        && current
            .mutable_i64_operands
            .iter()
            .zip(&expected.mutable_i64_operands)
            .all(|(actual, recorded)| {
                actual.operand_ordinal == recorded.operand_ordinal
                    && actual.pre_value == recorded.pre_value
            });
    current.operation_tag == expected.operation_tag
        && current.provider == expected.provider
        && current.scalar_operands == expected.scalar_operands
        && current.byte_operands == expected.byte_operands
        && current.path_like_operands == expected.path_like_operands
        && current.rooted_path_operand_resolutions == expected.rooted_path_operand_resolutions
        && current.mutable_byte_operand_resolutions == expected.mutable_byte_operand_resolutions
        && current.mutable_i64_operand_resolutions == expected.mutable_i64_operand_resolutions
        && current.logical_handle_inputs == expected.logical_handle_inputs
        && byte_pre_matches
        && scalar_pre_matches
}

fn filesystem_logical_handle_halt(
    error: filesystem_logical_handles::FilesystemLogicalHandleError,
) -> Halt {
    use filesystem_logical_handles::FilesystemLogicalHandleError as Error;
    match error {
        Error::IdentityExhausted => {
            Halt::Resource("filesystem logical-handle identity space exhausted".to_owned())
        }
        Error::LiveProviderTokenCollision { kind, raw } => Halt::Trap(format!(
            "filesystem provider reused live {kind:?} token `{raw}`"
        )),
        Error::BorrowSourceMismatch { raw } => Halt::Trap(format!(
            "filesystem provider reused native token `{raw}` for a different descriptor source"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::filesystem_preparation::{
        FilesystemTransferCountError, MAX_FILESYSTEM_TRANSFER_BYTES,
        PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandlePlan,
        PreparedI64Output, PreparedTransferCount, checked_filesystem_transfer_count,
    };
    use super::*;
    use crate::{FilesystemGrantRootIdentity, FilesystemScalarOperandValue};

    fn evaluator_with_pending_attempt(program: &TypedTrees) -> Evaluator<'_> {
        let mut evaluator = Evaluator::new(program, &[]);
        evaluator
            .filesystem_operation_attempts
            .push(FilesystemOperationAttempt::pending(
                1,
                FilesystemObservationProvider::Virtual,
            ));
        evaluator
    }

    fn returned_attempt(operation: FilesystemHostOperation) -> FilesystemOperationAttempt {
        let mut attempt = FilesystemOperationAttempt::pending(
            operation.operation_tag(),
            FilesystemObservationProvider::RealScoped,
        );
        attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(0),
            post_error: 0,
        });
        attempt
    }

    #[test]
    fn replay_cursor_rejects_reordered_extra_and_missing_events() {
        let program = TypedTrees::default();
        let expected = returned_attempt(FilesystemHostOperation::Open);
        let mut evaluator = Evaluator::new(&program, &[]);
        evaluator.filesystem_replay = Some(crate::FilesystemReplay {
            attempts: vec![expected.clone()].into(),
            expected_included_sources: std::sync::Arc::from([]),
        });

        assert!(matches!(
            evaluator.expected_filesystem_replay_attempt(0, FilesystemHostOperation::Open),
            Ok(Some(actual)) if actual == expected
        ));
        assert!(matches!(
            evaluator.expected_filesystem_replay_attempt(0, FilesystemHostOperation::Read),
            Err(Halt::Trap(message)) if message.contains("changed order")
        ));
        assert!(matches!(
            evaluator.expected_filesystem_replay_attempt(1, FilesystemHostOperation::Open),
            Err(Halt::Trap(message)) if message.contains("extra event")
        ));
        assert!(matches!(
            evaluator.finish_filesystem_replay(),
            Err(Halt::Trap(message)) if message.contains("record contains 1")
        ));
        evaluator.filesystem_operation_attempts.push(expected);
        assert!(evaluator.finish_filesystem_replay().is_ok());
    }

    #[test]
    fn replay_preparation_comparison_rejects_changed_input_lanes() {
        let mut current = returned_attempt(FilesystemHostOperation::Open);
        current.scalar_operands.push(FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I32(0),
        });
        let mut changed = current.clone();
        changed.scalar_operands[0].value = FilesystemScalarOperandValue::I32(1);
        assert!(replay_prepared_inputs_match(&current, &current));
        assert!(!replay_prepared_inputs_match(&current, &changed));

        changed = current.clone();
        changed
            .rooted_path_operand_resolutions
            .push(FilesystemRootedPathOperandResolution {
                operand_ordinal: 0,
                root: FilesystemGrantRootIdentity::new(1).unwrap(),
                relative_path: b"changed.omg".to_vec(),
            });
        assert!(!replay_prepared_inputs_match(&current, &changed));
    }

    #[test]
    fn transfer_count_rejects_wrap_and_unbounded_allocation() {
        assert_eq!(
            checked_filesystem_transfer_count(-1),
            Err(FilesystemTransferCountError::NegativeOrUnrepresentable)
        );
        assert_eq!(
            checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64 + 1),
            Err(FilesystemTransferCountError::ExceedsEvaluatorLimit)
        );
    }

    #[test]
    fn transfer_count_accepts_the_closed_interval_through_the_limit() {
        assert_eq!(
            checked_filesystem_transfer_count(0),
            Ok(PreparedTransferCount { raw: 0, host: 0 })
        );
        assert_eq!(
            checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64),
            Ok(PreparedTransferCount {
                raw: MAX_FILESYSTEM_TRANSFER_BYTES as u64,
                host: MAX_FILESYSTEM_TRANSFER_BYTES,
            })
        );
    }

    #[test]
    fn observation_evidence_budget_accepts_exact_limit_and_rejects_overflow() {
        assert_eq!(
            checked_observation_evidence_total(MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 1, 1),
            Some(MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES)
        );
        assert_eq!(
            checked_observation_evidence_total(MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES, 1),
            None
        );
        assert_eq!(checked_observation_evidence_total(usize::MAX, 1), None);
    }

    fn mutable_byte_output(bytes: &[u8]) -> PreparedByteOutput {
        PreparedByteOutput::Array(
            bytes
                .iter()
                .map(|byte| Value::Int(i64::from(*byte)).cell())
                .collect(),
        )
    }

    #[test]
    fn mutable_resolution_prefix_survives_its_own_capacity_failure() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        let output = mutable_byte_output(&[3, 1, 4]);

        evaluator
            .record_prepared_filesystem_mutable_byte_operand_resolution(0, 5, &output)
            .unwrap_or_else(|_| panic!("resolution fixture must be representable"));
        evaluator.record_prepared_filesystem_mutable_i64_operand_resolution(0, 6, -9);
        assert!(output.require_capacity(4).is_err());

        let attempt = &evaluator.filesystem_operation_attempts[0];
        assert_eq!(attempt.mutable_byte_operand_resolutions.len(), 1);
        assert_eq!(
            attempt.mutable_byte_operand_resolutions[0].operand_ordinal(),
            5
        );
        assert_eq!(
            attempt.mutable_byte_operand_resolutions[0].bytes(),
            &[3, 1, 4]
        );
        assert_eq!(attempt.mutable_i64_operand_resolutions.len(), 1);
        assert_eq!(
            attempt.mutable_i64_operand_resolutions[0].operand_ordinal(),
            6
        );
        assert_eq!(attempt.mutable_i64_operand_resolutions[0].value(), -9);
        assert!(attempt.mutable_byte_operands.is_empty());
        assert!(attempt.mutable_i64_operands.is_empty());
        assert_eq!(evaluator.filesystem_observation_evidence_bytes, 3);
    }

    #[test]
    fn completed_mutable_plan_accepts_alias_drift_and_retains_all_three_byte_snapshots() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        let output = mutable_byte_output(&[1, 2]);
        evaluator
            .record_prepared_filesystem_mutable_byte_operand_resolution(0, 1, &output)
            .unwrap_or_else(|_| panic!("resolution fixture must be representable"));

        output
            .write(&[8, 9])
            .unwrap_or_else(|_| panic!("alias fixture write must fit"));
        let call = PreparedFilesystemCall::Read {
            fd: 3,
            buffer: output.clone(),
            count: PreparedTransferCount { raw: 1, host: 1 },
        };
        let (scalar_operands, byte_operands, path_like_operands) = call.operand_observation_plan();
        evaluator.filesystem_operation_attempts[0].scalar_operands = scalar_operands.clone();
        let plan = call
            .mutable_observation_plan()
            .unwrap_or_else(|_| panic!("mutable fixture must be representable"));
        evaluator
            .record_operand_observations(
                0,
                scalar_operands,
                byte_operands,
                path_like_operands,
                &[],
                &plan,
            )
            .unwrap_or_else(|_| panic!("matching resolution roles must validate"));

        assert_eq!(
            evaluator.filesystem_operation_attempts[0].mutable_byte_operand_resolutions[0].bytes(),
            &[1, 2],
            "resolution-time contents must not be overwritten by provider pre-state"
        );
        assert_eq!(
            evaluator.filesystem_operation_attempts[0].mutable_byte_operands[0].pre_bytes(),
            &[8, 9]
        );
        assert_eq!(
            evaluator.filesystem_observation_evidence_bytes, 6,
            "resolution, provider pre-state, and provider post-state share one sponsor"
        );

        output
            .write(&[7])
            .unwrap_or_else(|_| panic!("provider fixture write must fit"));
        evaluator
            .complete_mutable_observations(0, &plan)
            .unwrap_or_else(|_| panic!("completed fixture must be representable"));
        let attempt = &evaluator.filesystem_operation_attempts[0];
        assert_eq!(attempt.mutable_byte_operand_resolutions[0].bytes(), &[1, 2]);
        assert_eq!(attempt.mutable_byte_operands[0].pre_bytes(), &[8, 9]);
        assert_eq!(attempt.mutable_byte_operands[0].post_bytes(), &[7, 9]);
    }

    #[test]
    fn completed_rooted_path_sidecar_requires_exact_portable_coordinates() {
        let program = TypedTrees::default();
        let root = crate::FilesystemGrantRootIdentity::new(1)
            .unwrap_or_else(|| panic!("fixture root identity is nonzero"));
        let resolution = FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root,
            relative_path: b"inputs/table.txt".to_vec(),
        };
        let call = PreparedFilesystemCall::Remove {
            path: b"/physical/source/inputs/table.txt".to_vec(),
        };
        let (scalar_operands, byte_operands, path_like_operands) = call.operand_observation_plan();
        let mutable_plan = call
            .mutable_observation_plan()
            .unwrap_or_else(|_| panic!("remove has no mutable observation failure"));

        let mut matching = evaluator_with_pending_attempt(&program);
        matching
            .record_prepared_filesystem_rooted_path_operand_resolution(0, resolution.clone())
            .unwrap_or_else(|_| panic!("rooted fixture fits evidence sponsor"));
        matching
            .record_operand_observations(
                0,
                scalar_operands.clone(),
                byte_operands.clone(),
                path_like_operands.clone(),
                std::slice::from_ref(&resolution),
                &mutable_plan,
            )
            .unwrap_or_else(|_| panic!("exact rooted sidecar must validate"));

        let mut mismatching = evaluator_with_pending_attempt(&program);
        mismatching
            .record_prepared_filesystem_rooted_path_operand_resolution(0, resolution)
            .unwrap_or_else(|_| panic!("rooted fixture fits evidence sponsor"));
        let wrong = FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root,
            relative_path: b"inputs/other.txt".to_vec(),
        };
        assert!(
            mismatching
                .record_operand_observations(
                    0,
                    scalar_operands,
                    byte_operands,
                    path_like_operands,
                    &[wrong],
                    &mutable_plan,
                )
                .is_err(),
            "physical provider bytes must not substitute for exact portable rooted coordinates"
        );
    }

    #[test]
    fn rooted_path_resolution_budget_failure_is_atomic() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_observation_evidence_bytes =
            MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 2;
        let resolution = FilesystemRootedPathOperandResolution {
            operand_ordinal: 0,
            root: crate::FilesystemGrantRootIdentity::new(1)
                .unwrap_or_else(|| panic!("fixture root identity is nonzero")),
            relative_path: b"abc".to_vec(),
        };

        assert!(
            evaluator
                .record_prepared_filesystem_rooted_path_operand_resolution(0, resolution)
                .is_err()
        );
        assert_eq!(
            evaluator.filesystem_observation_evidence_bytes,
            MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 2
        );
        assert!(
            evaluator.filesystem_operation_attempts[0]
                .rooted_path_operand_resolutions
                .is_empty()
        );
    }

    #[test]
    fn returned_path_rows_retain_exact_bytes_and_completeness() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempt_stack.push(0);
        evaluator
            .record_returned_path_observation(
                1,
                FilesystemReturnedPathKind::ReadLinkPayload,
                FilesystemReturnedPathCompleteness::Complete,
                b"ab",
            )
            .unwrap_or_else(|_| panic!("complete returned path fits sponsor"));
        evaluator
            .record_returned_path_observation(
                1,
                FilesystemReturnedPathKind::ReadLinkPayload,
                FilesystemReturnedPathCompleteness::LimitReached,
                b"prefix",
            )
            .unwrap_or_else(|_| panic!("limited returned path fits sponsor"));
        let [returned, limited] = &evaluator.filesystem_operation_attempts[0].returned_paths[..]
        else {
            panic!("fixture retains both returned paths")
        };
        assert_eq!(
            returned.kind(),
            crate::FilesystemReturnedPathKind::ReadLinkPayload
        );
        assert_eq!(returned.operand_ordinal(), 1);
        assert_eq!(returned.bytes(), b"ab");
        assert_eq!(
            returned.completeness(),
            crate::FilesystemReturnedPathCompleteness::Complete
        );
        assert_eq!(
            limited.completeness(),
            crate::FilesystemReturnedPathCompleteness::LimitReached
        );
        assert_eq!(limited.bytes(), b"prefix");
        assert_eq!(evaluator.filesystem_observation_evidence_bytes, 8);
    }

    #[test]
    fn virtual_read_link_provider_distinguishes_exact_fit_from_truncation() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempt_stack.push(0);
        evaluator
            .virtual_symlinks
            .insert(b"exact".to_vec(), b"abcd".to_vec());
        evaluator
            .virtual_symlinks
            .insert(b"limited".to_vec(), b"abcdef".to_vec());

        let exact_output = mutable_byte_output(&[9; 6]);
        let exact_result = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::ReadLink {
                path: b"exact".to_vec(),
                buffer: exact_output.clone(),
                count: PreparedTransferCount { raw: 4, host: 4 },
            })
            .unwrap_or_else(|_| panic!("exact-fit read_link succeeds"));
        assert!(matches!(exact_result, Value::Int(4)));
        assert_eq!(
            exact_output
                .snapshot()
                .unwrap_or_else(|_| panic!("exact output remains readable")),
            b"abcd\x09\x09"
        );

        let limited_output = mutable_byte_output(&[9; 6]);
        let limited_result = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::ReadLink {
                path: b"limited".to_vec(),
                buffer: limited_output.clone(),
                count: PreparedTransferCount { raw: 4, host: 4 },
            })
            .unwrap_or_else(|_| panic!("truncated read_link succeeds"));
        assert!(matches!(limited_result, Value::Int(4)));
        assert_eq!(
            limited_output
                .snapshot()
                .unwrap_or_else(|_| panic!("limited output remains readable")),
            b"abcd\x09\x09"
        );

        let [exact, limited] = &evaluator.filesystem_operation_attempts[0].returned_paths[..]
        else {
            panic!("provider records both successful writes")
        };
        assert_eq!(exact.bytes(), b"abcd");
        assert_eq!(
            exact.completeness(),
            FilesystemReturnedPathCompleteness::Complete
        );
        assert_eq!(limited.bytes(), b"abcd");
        assert_eq!(
            limited.completeness(),
            FilesystemReturnedPathCompleteness::LimitReached
        );
    }

    #[test]
    fn returned_path_budget_failure_is_atomic() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempt_stack.push(0);
        evaluator.filesystem_observation_evidence_bytes =
            MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 1;

        assert!(
            evaluator
                .record_returned_path_observation(
                    1,
                    FilesystemReturnedPathKind::ReadLinkPayload,
                    FilesystemReturnedPathCompleteness::Complete,
                    b"ab",
                )
                .is_err()
        );
        assert_eq!(
            evaluator.filesystem_observation_evidence_bytes,
            MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES - 1
        );
        assert!(
            evaluator.filesystem_operation_attempts[0]
                .returned_paths
                .is_empty()
        );
    }

    #[test]
    fn virtual_file_reads_retain_only_exact_observed_bytes() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempt_stack.push(0);
        evaluator
            .virtual_files
            .insert(b"input".to_vec(), b"abcdef".to_vec());
        let descriptor = evaluator.virtual_open(b"input".to_vec(), false, false);

        let sequential_output = mutable_byte_output(&[9; 8]);
        let sequential = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::Read {
                fd: descriptor,
                buffer: sequential_output.clone(),
                count: PreparedTransferCount { raw: 4, host: 4 },
            })
            .unwrap_or_else(|_| panic!("sequential read succeeds"));
        assert!(matches!(sequential, Value::Int(4)));
        assert_eq!(
            sequential_output
                .snapshot()
                .unwrap_or_else(|_| panic!("sequential output remains readable")),
            b"abcd\x09\x09\x09\x09"
        );

        let positioned_output = mutable_byte_output(&[9; 8]);
        let positioned = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::ReadAt {
                fd: descriptor,
                buffer: positioned_output,
                count: PreparedTransferCount { raw: 3, host: 3 },
                offset: 1,
            })
            .unwrap_or_else(|_| panic!("positioned read succeeds"));
        assert!(matches!(positioned, Value::Int(3)));

        let tail_output = mutable_byte_output(&[9; 8]);
        let tail = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::Read {
                fd: descriptor,
                buffer: tail_output,
                count: PreparedTransferCount { raw: 8, host: 8 },
            })
            .unwrap_or_else(|_| panic!("tail read succeeds"));
        assert!(matches!(tail, Value::Int(2)));

        let eof_output = mutable_byte_output(&[9; 8]);
        let eof = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::Read {
                fd: descriptor,
                buffer: eof_output,
                count: PreparedTransferCount { raw: 8, host: 8 },
            })
            .unwrap_or_else(|_| panic!("EOF read succeeds"));
        assert!(matches!(eof, Value::Int(0)));

        let failed_output = mutable_byte_output(&[9; 8]);
        let failed = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::Read {
                fd: -1,
                buffer: failed_output.clone(),
                count: PreparedTransferCount { raw: 8, host: 8 },
            })
            .unwrap_or_else(|_| panic!("unknown descriptor is a provider error"));
        assert!(matches!(failed, Value::Int(-1)));
        assert_eq!(
            failed_output
                .snapshot()
                .unwrap_or_else(|_| panic!("failed output remains readable")),
            &[9; 8]
        );

        let rows = &evaluator.filesystem_operation_attempts[0].observed_byte_regions;
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| row.output_operand_ordinal() == 1));
        assert_eq!(
            rows[0].kind(),
            FilesystemObservedByteRegionKind::SequentialFileRead
        );
        assert_eq!(
            rows[1].kind(),
            FilesystemObservedByteRegionKind::PositionedFileRead
        );
        assert_eq!(
            rows[2].kind(),
            FilesystemObservedByteRegionKind::SequentialFileRead
        );
        assert_eq!(
            rows[3].kind(),
            FilesystemObservedByteRegionKind::SequentialFileRead
        );
        assert_eq!(rows[0].offset(), 0);
        assert_eq!(rows[0].length(), 4);
        assert_eq!(rows[1].length(), 3);
        assert_eq!(rows[2].length(), 2);
        assert_eq!(rows[3].length(), 0);
        assert_eq!(
            evaluator.filesystem_observation_evidence_bytes, 0,
            "semantic regions reference the already-custodied mutable post-state"
        );
    }

    #[test]
    fn observed_byte_region_validation_binds_result_kind_and_post_carrier() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempts[0]
            .observed_byte_regions
            .push(FilesystemObservedByteRegion {
                output_operand_ordinal: 1,
                kind: FilesystemObservedByteRegionKind::SequentialFileRead,
                offset: 0,
                length: 4,
            });
        evaluator.filesystem_operation_attempts[0]
            .mutable_byte_operands
            .push(crate::FilesystemMutableByteOperand {
                operand_ordinal: 1,
                pre_bytes: vec![9; 6],
                post_bytes: b"abcd\x09\x09".to_vec(),
            });

        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::Read, 4)
            .unwrap_or_else(|_| panic!("exact read region validates"));
        assert!(
            evaluator
                .validate_observed_byte_regions(0, FilesystemHostOperation::Read, 3)
                .is_err(),
            "scalar result must equal semantic region length"
        );
        assert!(
            evaluator
                .validate_observed_byte_regions(0, FilesystemHostOperation::ReadAt, 4)
                .is_err(),
            "positioned and sequential read regions are distinct"
        );
        evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].kind =
            FilesystemObservedByteRegionKind::DirectoryRecords;
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::ReadDir, 4)
            .unwrap_or_else(|_| panic!("directory record region validates"));

        evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].kind =
            FilesystemObservedByteRegionKind::FindEntry;
        evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].length =
            FIND_DATA_OUTPUT_BYTES;
        evaluator.filesystem_operation_attempts[0].mutable_byte_operands[0]
            .pre_bytes
            .resize(FIND_DATA_OUTPUT_BYTES, 9);
        evaluator.filesystem_operation_attempts[0].mutable_byte_operands[0]
            .post_bytes
            .resize(FIND_DATA_OUTPUT_BYTES, 0);
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::FindFirst, 7)
            .unwrap_or_else(|_| panic!("find_first record region validates"));
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::FindNext, 1)
            .unwrap_or_else(|_| panic!("find_next record region validates"));
        evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].length = 0;
        evaluator
            .validate_observed_byte_regions(0, FilesystemHostOperation::FindNext, 0)
            .unwrap_or_else(|_| panic!("find_next empty region validates"));

        evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].kind =
            FilesystemObservedByteRegionKind::SequentialFileRead;
        evaluator.filesystem_operation_attempts[0].observed_byte_regions[0].length = 4;
        evaluator.filesystem_operation_attempts[0]
            .mutable_byte_operands
            .clear();
        assert!(
            evaluator
                .validate_observed_byte_regions(0, FilesystemHostOperation::Read, 4)
                .is_err(),
            "semantic region requires its retained post-carrier"
        );
    }

    #[test]
    fn virtual_directory_enumeration_designates_exact_output_regions() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempt_stack.push(0);
        evaluator.virtual_dirs.insert(b"dir".to_vec());
        evaluator
            .virtual_files
            .insert(b"dir/entry".to_vec(), b"payload".to_vec());
        let descriptor = evaluator.virtual_open_flags(b"dir".to_vec(), 0);
        assert!(descriptor >= 0);

        let directory_output = mutable_byte_output(&[9; 512]);
        let directory = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::ReadDir {
                fd: descriptor,
                buffer: directory_output.clone(),
                count: PreparedTransferCount {
                    raw: 512,
                    host: 512,
                },
                position: PreparedI64Output::test_fixture(0),
            })
            .unwrap_or_else(|_| panic!("directory enumeration succeeds"));
        let Value::Int(directory_length) = directory else {
            panic!("read_dir returns a byte length")
        };
        assert!(directory_length > 0);
        let directory_length = usize::try_from(directory_length)
            .unwrap_or_else(|_| panic!("directory fixture length is representable"));
        let directory_bytes = directory_output
            .snapshot()
            .unwrap_or_else(|_| panic!("directory output remains readable"));
        assert!(
            directory_bytes[directory_length..]
                .iter()
                .all(|byte| *byte == 9)
        );

        let eof_output = mutable_byte_output(&[9; 512]);
        let eof = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::ReadDir {
                fd: descriptor,
                buffer: eof_output.clone(),
                count: PreparedTransferCount {
                    raw: 512,
                    host: 512,
                },
                position: PreparedI64Output::test_fixture(i64::MAX),
            })
            .unwrap_or_else(|_| panic!("directory EOF succeeds"));
        assert!(matches!(eof, Value::Int(0)));
        assert_eq!(
            eof_output
                .snapshot()
                .unwrap_or_else(|_| panic!("EOF output remains readable")),
            &[9; 512]
        );

        let first_output = mutable_byte_output(&[9; 322]);
        let first = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::FindFirst {
                pattern: b"dir/*".to_vec(),
                data: first_output.clone(),
            })
            .unwrap_or_else(|_| panic!("find_first succeeds"));
        let Value::Int(find_handle) = first else {
            panic!("find_first returns its handle")
        };
        assert!(find_handle >= 0);
        assert_eq!(
            &first_output
                .snapshot()
                .unwrap_or_else(|_| panic!("first find output remains readable"))
                [FIND_DATA_OUTPUT_BYTES..],
            &[9, 9]
        );

        let next_output = mutable_byte_output(&[9; 322]);
        let next = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::FindNext {
                handle: find_handle,
                data: next_output,
            })
            .unwrap_or_else(|_| panic!("find_next succeeds"));
        assert!(matches!(next, Value::Int(1)));

        let empty_output = mutable_byte_output(&[9; 322]);
        let empty = evaluator
            .serve_filesystem_call(PreparedFilesystemCall::FindNext {
                handle: -1,
                data: empty_output.clone(),
            })
            .unwrap_or_else(|_| panic!("unknown find handle returns no entry"));
        assert!(matches!(empty, Value::Int(0)));
        assert_eq!(
            empty_output
                .snapshot()
                .unwrap_or_else(|_| panic!("empty find output remains readable")),
            &[9; 322]
        );

        let rows = &evaluator.filesystem_operation_attempts[0].observed_byte_regions;
        assert_eq!(rows.len(), 5);
        assert_eq!(
            rows[0].kind(),
            FilesystemObservedByteRegionKind::DirectoryRecords
        );
        assert_eq!(rows[0].length(), directory_length);
        assert_eq!(
            rows[1].kind(),
            FilesystemObservedByteRegionKind::DirectoryRecords
        );
        assert_eq!(rows[1].length(), 0);
        assert!(rows[2..].iter().all(|row| {
            row.kind() == FilesystemObservedByteRegionKind::FindEntry
                && row.output_operand_ordinal() == 1
                && row.offset() == 0
        }));
        assert_eq!(rows[2].length(), FIND_DATA_OUTPUT_BYTES);
        assert_eq!(rows[3].length(), FIND_DATA_OUTPUT_BYTES);
        assert_eq!(rows[4].length(), 0);
    }

    #[test]
    fn observed_byte_region_rejects_out_of_bounds_without_a_row() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.filesystem_operation_attempt_stack.push(0);
        let output = mutable_byte_output(&[9]);

        assert!(
            evaluator
                .record_observed_byte_region(
                    1,
                    FilesystemObservedByteRegionKind::SequentialFileRead,
                    &output,
                    0,
                    2,
                )
                .is_err()
        );
        assert!(
            evaluator.filesystem_operation_attempts[0]
                .observed_byte_regions
                .is_empty()
        );
    }

    #[test]
    fn incremental_logical_handle_inputs_retain_exact_resolution_prefix() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        let descriptor = evaluator
            .filesystem_logical_handles
            .create(FilesystemLogicalHandleKind::Descriptor, 7)
            .unwrap();

        evaluator.record_prepared_filesystem_logical_handle_input(
            0,
            0,
            FilesystemLogicalHandleKind::Descriptor,
            7,
            false,
        );
        evaluator.record_prepared_filesystem_logical_handle_input(
            0,
            1,
            FilesystemLogicalHandleKind::Find,
            7,
            false,
        );
        evaluator.record_prepared_filesystem_logical_handle_input(
            0,
            2,
            FilesystemLogicalHandleKind::Native,
            0,
            true,
        );
        evaluator.record_prepared_filesystem_logical_handle_input(
            0,
            3,
            FilesystemLogicalHandleKind::Native,
            0,
            false,
        );

        assert_eq!(
            evaluator.filesystem_operation_attempts[0].logical_handle_inputs,
            vec![
                FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind: FilesystemLogicalHandleKind::Descriptor,
                    resolution: FilesystemLogicalHandleInputResolution::Resolved(descriptor),
                },
                FilesystemLogicalHandleInput {
                    operand_ordinal: 1,
                    kind: FilesystemLogicalHandleKind::Find,
                    resolution: FilesystemLogicalHandleInputResolution::Unknown,
                },
                FilesystemLogicalHandleInput {
                    operand_ordinal: 2,
                    kind: FilesystemLogicalHandleKind::Native,
                    resolution: FilesystemLogicalHandleInputResolution::Null,
                },
                FilesystemLogicalHandleInput {
                    operand_ordinal: 3,
                    kind: FilesystemLogicalHandleKind::Native,
                    resolution: FilesystemLogicalHandleInputResolution::Unknown,
                },
            ],
            "a later preparation halt must retain every already typed handle input",
        );
    }

    #[test]
    fn completed_logical_handle_plan_cross_checks_without_overwriting_evidence() {
        let program = TypedTrees::default();
        let mut evaluator = evaluator_with_pending_attempt(&program);
        evaluator.record_prepared_filesystem_logical_handle_input(
            0,
            0,
            FilesystemLogicalHandleKind::Descriptor,
            7,
            false,
        );
        let retained = evaluator.filesystem_operation_attempts[0]
            .logical_handle_inputs
            .clone();
        let matching = PreparedFilesystemLogicalHandlePlan {
            inputs: vec![PreparedFilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                raw: 7,
                null_allowed: false,
            }],
            input_success: None,
            output: None,
            retirement: None,
        };
        assert!(
            evaluator
                .validate_incremental_logical_handle_inputs(0, &matching)
                .is_ok()
        );
        assert_eq!(
            evaluator.filesystem_operation_attempts[0].logical_handle_inputs,
            retained,
        );

        let disagreement = PreparedFilesystemLogicalHandlePlan {
            inputs: vec![PreparedFilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Find,
                raw: 7,
                null_allowed: false,
            }],
            ..matching
        };
        assert!(
            evaluator
                .validate_incremental_logical_handle_inputs(0, &disagreement)
                .is_err()
        );
        assert_eq!(
            evaluator.filesystem_operation_attempts[0].logical_handle_inputs,
            retained,
        );
    }
}
