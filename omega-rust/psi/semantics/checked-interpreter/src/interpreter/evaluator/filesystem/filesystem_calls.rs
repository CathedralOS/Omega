//! Trying and serving one filesystem call: the entry the evaluator takes
//! for every filesystem operation and the dispatch over each call kind.
//!
//! Every operation dispatches from `serve_filesystem_call` to its `serve_*`
//! method in `descriptor_calls`, `path_calls` or `directory_calls`.

mod descriptor_calls;
mod directory_calls;
mod path_calls;

use crate::interpreter::evaluator::Evaluator;
use crate::interpreter::evaluator::{
    EvalResult, ExpressionHandle, FilesystemEvaluationHaltKind, FilesystemHostOperation,
    FilesystemHostResultKind, FilesystemObservationProvider, FilesystemOperationAttempt,
    FilesystemOperationAttemptOutcome, FilesystemOperationResult, Frame, Halt,
    MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES, PreparedFilesystemCall, Value,
};

pub(crate) fn checked_observation_evidence_total(
    current: usize,
    additional: usize,
) -> Option<usize> {
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
    pub(crate) fn try_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<Value> {
        self.charge_filesystem_operation_attempt()?;
        let attempt_index = self.filesystem_operation_attempts.len();
        let provider = match self.real_fs.as_ref() {
            None => FilesystemObservationProvider::Virtual,
            Some(filesystem) if filesystem.is_scoped() => FilesystemObservationProvider::RealScoped,
            Some(_) => FilesystemObservationProvider::RealUnscoped,
        };
        self.filesystem_operation_attempts
            .push(FilesystemOperationAttempt::pending(
                operation.operation_tag(),
                provider,
            ));
        self.filesystem_operation_attempt_stack.push(attempt_index);
        let mut outcome = (|| {
            let call = self.prepare_filesystem_call(operation, arguments, frame)?;
            call.validate_output_carriers()?;
            let logical_handle_plan = call.logical_handle_plan();
            self.validate_incremental_logical_handle_inputs(attempt_index, &logical_handle_plan)?;
            self.reject_cross_domain_logical_handle_inputs(&logical_handle_plan)?;
            self.reject_rooted_final_path_result(attempt_index, &call)?;
            let live_handle_lease =
                self.reserve_prepared_live_filesystem_handle(&logical_handle_plan)?;
            let value = self.serve_filesystem_call(call)?;
            Ok((value, logical_handle_plan, live_handle_lease))
        })();
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
            Ok((value, logical_handle_plan, mut live_handle_lease)) => {
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
                if let Err(halt) = self.complete_logical_handle_observations(
                    attempt_index,
                    &logical_handle_plan,
                    result,
                    &mut live_handle_lease,
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
                // Only a fully observed attempt may mint or retire output writers.
                self.note_completed_build_output_attempt(attempt_index);
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

    /// Drive a value-returning filesystem operation against the selected provider.
    /// Preparation has evaluated every operand once and validated output places.
    pub(super) fn serve_filesystem_call(
        &mut self,
        call: PreparedFilesystemCall,
    ) -> EvalResult<Value> {
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
            PreparedFilesystemCall::OpenPathHandle { .. } => self.serve_open_path_handle(call)?,
            PreparedFilesystemCall::OpenCreate { .. } => self.serve_open_create(call)?,
            PreparedFilesystemCall::Read { .. } => self.serve_read(call)?,
            PreparedFilesystemCall::Write { .. } => self.serve_write(call)?,
            PreparedFilesystemCall::ReadAt { .. } => self.serve_read_at(call)?,
            PreparedFilesystemCall::WriteAt { .. } => self.serve_write_at(call)?,
            PreparedFilesystemCall::Close { .. } => self.serve_close(call)?,
            PreparedFilesystemCall::CloseHandle { .. } => self.serve_close_handle(call)?,
            PreparedFilesystemCall::Duplicate { .. } => self.serve_duplicate(call)?,
            PreparedFilesystemCall::LockFile { .. } => self.serve_lock_file(call)?,
            PreparedFilesystemCall::LockFileEx { .. } => self.serve_lock_file_ex(call)?,
            PreparedFilesystemCall::UnlockFile { .. } => self.serve_unlock_file(call)?,
            PreparedFilesystemCall::GetLastError => i64::from(self.virtual_errno),
            // `remove_name` is the TRUSTED plain-path twin (D-at trust class,
            // the create_dir_name precedent): the arg bytes ARE the path, so
            // both spellings share one model.
            PreparedFilesystemCall::Remove { .. } | PreparedFilesystemCall::RemoveName { .. } => {
                self.serve_remove(call)?
            }
            PreparedFilesystemCall::Seek { .. } => self.serve_seek(call)?,
            PreparedFilesystemCall::SetLen { .. } => self.serve_set_len(call)?,
            PreparedFilesystemCall::SetFilePermissions { .. } => {
                self.serve_set_file_permissions(call)?
            }
            PreparedFilesystemCall::SetFileTimes { .. } => self.serve_set_file_times(call)?,
            PreparedFilesystemCall::Sync { .. } | PreparedFilesystemCall::SyncData { .. } => {
                self.serve_sync(call)?
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
            PreparedFilesystemCall::CreateDir { .. }
            | PreparedFilesystemCall::CreateDirName { .. } => self.serve_create_dir(call)?,
            PreparedFilesystemCall::RemoveDir { .. }
            | PreparedFilesystemCall::RemoveDirName { .. } => self.serve_remove_dir(call)?,
            PreparedFilesystemCall::OpenAt { .. } => self.serve_open_at(call)?,
            PreparedFilesystemCall::UnlinkAt { .. } => self.serve_unlink_at(call)?,
            PreparedFilesystemCall::SetPermissions { .. } => self.serve_set_permissions(call)?,
            PreparedFilesystemCall::ChangeOwner { .. }
            | PreparedFilesystemCall::ChangeOwnerNoFollow { .. } => {
                self.serve_change_owner(call)?
            }
            PreparedFilesystemCall::ChangeFileOwner { .. } => self.serve_change_file_owner(call)?,
            PreparedFilesystemCall::Rename { .. } => self.serve_rename(call)?,
            PreparedFilesystemCall::HardLink { .. } => self.serve_hard_link(call)?,
            PreparedFilesystemCall::CreateHardLink { .. } => self.serve_create_hard_link(call)?,
            PreparedFilesystemCall::GetOsfHandle { .. } => self.serve_get_osf_handle(call)?,
            PreparedFilesystemCall::FinalPathNameByHandle { .. } => {
                self.serve_final_path_name_by_handle(call)?
            }
            PreparedFilesystemCall::SetFileTime { .. } => self.serve_set_file_time(call)?,
            PreparedFilesystemCall::Symlink { .. } => self.serve_symlink(call)?,
            PreparedFilesystemCall::ReadLink { .. } => self.serve_read_link(call)?,
            PreparedFilesystemCall::Canonicalize { .. } => self.serve_canonicalize(call)?,
            PreparedFilesystemCall::ReadDir { .. } => self.serve_read_dir(call)?,
            PreparedFilesystemCall::FindFirst { .. } => self.serve_find_first(call)?,
            PreparedFilesystemCall::FindNext { .. } => self.serve_find_next(call)?,
            PreparedFilesystemCall::FindClose { .. } => self.serve_find_close(call)?,
            PreparedFilesystemCall::ReadMetadata { .. } => self.serve_read_metadata(call)?,
            PreparedFilesystemCall::ReadFileMetadata { .. } => {
                self.serve_read_file_metadata(call)?
            }
            PreparedFilesystemCall::ReadSymlinkMetadata { .. } => {
                self.serve_read_symlink_metadata(call)?
            }
        };
        Ok(Value::Int(result))
    }
}
