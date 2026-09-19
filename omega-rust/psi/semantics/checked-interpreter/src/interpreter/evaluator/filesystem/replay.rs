//! Replaying a recorded filesystem call: the expected attempt, serving a
//! replayed or executed-replay call, and finishing the replay.

use crate::interpreter::evaluator::{
    EvalResult, Evaluator, FilesystemHostOperation, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, Halt, PreparedFilesystemCall,
    PreparedFilesystemMutableObservationPlan, Value, trap,
};

impl<'program> Evaluator<'program> {
    pub(crate) fn expected_filesystem_replay_attempt(
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

    pub(crate) fn serve_replayed_filesystem_call(
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
            FilesystemOperationResult::LogicalHandle(identity) => {
                if expected.logical_handle_output.is_some_and(|output| {
                    output.kind == crate::FilesystemLogicalHandleKind::Descriptor
                }) {
                    // Injected Source descriptors and executed Output descriptors
                    // share one raw-token namespace. A recorded logical identity
                    // is not a free provider token: another live Output can own
                    // that number when their lifetimes interleave.
                    let descriptor = self.virtual_next_fd;
                    self.virtual_next_fd = descriptor.checked_add(1).ok_or_else(|| {
                        Halt::Resource("filesystem replay descriptor space exhausted".to_owned())
                    })?;
                    i64::from(descriptor)
                } else {
                    i64::try_from(identity.get()).map_err(|_| {
                        Halt::Trap(format!(
                            "filesystem replay event {attempt_index} logical handle exceeds i64"
                        ))
                    })?
                }
            }
        };
        Ok(Value::Int(raw))
    }

    pub(crate) fn serve_executed_replay_filesystem_call(
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
}

pub(crate) fn replay_prepared_inputs_match(
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
