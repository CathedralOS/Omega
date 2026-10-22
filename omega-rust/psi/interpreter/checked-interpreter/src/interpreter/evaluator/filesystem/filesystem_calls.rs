//! Trying and serving one filesystem call: the entry the evaluator takes
//! for every Build filesystem operation and the dispatch over each call kind.
//!
//! Every operation dispatches from `serve_filesystem_call` to its `serve_*`
//! method in `descriptor_calls`, `path_calls` or `directory_calls`.

mod descriptor_calls;
mod directory_calls;
mod path_calls;

use crate::interpreter::evaluator::Evaluator;
use crate::interpreter::evaluator::{
    EvalResult, FilesystemEvaluationHaltKind, FilesystemHostOperation, FilesystemHostResultKind,
    FilesystemObservationProvider, FilesystemOperationAttempt, FilesystemOperationAttemptOutcome,
    FilesystemOperationResult, Halt, MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES,
    PreparedFilesystemCall, Value,
};

/// Dispatch rows of a `PreparedFilesystemCall` match: every dispatch row runs
/// the variant's method on the receiver with the whole call. Arms that bind
/// fields or take their own tails ride in `literal` ahead of the rows.
#[macro_export]
macro_rules! prepared_filesystem_call_dispatch {
    (
        $receiver:ident, $call:ident;
        literal { $( $pat:pat => $body:block $(,)? )* }
        dispatch { $( $($variant:ident)|+ => $method:ident ),* $(,)? }
    ) => {
        match $call {
            $( $pat => $body, )*
            $(
                $( PreparedFilesystemCall::$variant { .. } )|+ => {
                    $receiver.$method($call)?
                }
            )*
        }
    };
}

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
    /// evaluation itself invokes another filesystem operation. Preparation
    /// includes any explicit activation facet, not just filesystem operands.
    /// A provider evaluation halt aborts the entire build, so incomplete placeholders can
    /// never enter a successful measured result.
    pub(crate) fn evaluate_filesystem_operation(
        &mut self,
        operation: FilesystemHostOperation,
        prepare: impl FnOnce(&mut Self) -> EvalResult<PreparedFilesystemCall>,
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
            let call = prepare(self)?;
            let logical_handle_plan = call.logical_handle_plan();
            let captured = self.real_fs.as_ref().is_some_and(|filesystem| {
                filesystem.call_uses_captured_input(&call, &logical_handle_plan)
            });
            if captured {
                self.filesystem_operation_attempts[attempt_index].provider =
                    FilesystemObservationProvider::Captured;
            }
            call.validate_output_carriers()?;
            self.validate_incremental_logical_handle_inputs(attempt_index, &logical_handle_plan)?;
            self.reject_cross_domain_logical_handle_inputs(&logical_handle_plan)?;
            self.reject_rooted_final_path_result(attempt_index, &call)?;
            let live_handle_lease =
                self.reserve_prepared_live_filesystem_handle(&logical_handle_plan)?;
            let value = if captured {
                self.serve_captured_filesystem_call(call, &logical_handle_plan)?
            } else {
                self.serve_filesystem_call(call)?
            };
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
                        Halt::Trap(_) | Halt::Crash(_, _) => FilesystemEvaluationHaltKind::Trap,
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
                    Halt::Trap(_) | Halt::Crash(_, _) => FilesystemEvaluationHaltKind::Trap,
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
        let result: i64 = prepared_filesystem_call_dispatch!(
            self, call;
            literal {
                PreparedFilesystemCall::Create { path, mode: _ } => {
                    // O_WRONLY|O_CREAT|O_TRUNC: create/truncate, writable.
                    self.virtual_open(path.into_native()?, true, true) as i64
                }
                PreparedFilesystemCall::Open { path, flags } => {
                    self.virtual_open_flags(path.into_native()?, flags) as i64
                }
                PreparedFilesystemCall::GetLastError => { i64::from(self.virtual_errno) }
                PreparedFilesystemCall::Errno => {
                    // `read_errno()` (darwin `___error()` deref): the thread-local
                    // errno set by the most recent failing op. Not cleared on
                    // success (POSIX), so it is only meaningful right after a -1.
                    i64::from(self.virtual_errno)
                }
            }
            dispatch {
                OpenPathHandle => serve_open_path_handle,
                OpenCreate => serve_open_create,
                Read => serve_read,
                Write => serve_write,
                ReadAt => serve_read_at,
                WriteAt => serve_write_at,
                Close => serve_close,
                CloseHandle => serve_close_handle,
                Duplicate => serve_duplicate,
                LockFile => serve_lock_file,
                LockFileEx => serve_lock_file_ex,
                UnlockFile => serve_unlock_file,
                // `remove_name` is the TRUSTED plain-path twin (D-at trust
                // class, the create_dir_name precedent): the arg bytes ARE
                // the path, so both spellings share one model.
                Remove | RemoveName => serve_remove,
                Seek => serve_seek,
                SetLen => serve_set_len,
                SetFilePermissions => serve_set_file_permissions,
                SetFileTimes => serve_set_file_times,
                Sync | SyncData => serve_sync,
                // The trusted plain-name variant shares create_dir's
                // semantics (the arg bytes ARE the path -- the scratch
                // subslice excludes the native NUL, so both engines see
                // identical bytes).
                CreateDir | CreateDirName => serve_create_dir,
                RemoveDir | RemoveDirName => serve_remove_dir,
                OpenAt => serve_open_at,
                UnlinkAt => serve_unlink_at,
                SetPermissions => serve_set_permissions,
                ChangeOwner | ChangeOwnerNoFollow => serve_change_owner,
                ChangeFileOwner => serve_change_file_owner,
                Rename => serve_rename,
                HardLink => serve_hard_link,
                CreateHardLink => serve_create_hard_link,
                GetOsfHandle => serve_get_osf_handle,
                FinalPathNameByHandle => serve_final_path_name_by_handle,
                SetFileTime => serve_set_file_time,
                Symlink => serve_symlink,
                ReadLink => serve_read_link,
                Canonicalize => serve_canonicalize,
                ReadDir => serve_read_dir,
                FindFirst => serve_find_first,
                FindNext => serve_find_next,
                FindClose => serve_find_close,
                ReadMetadata => serve_read_metadata,
                ReadFileMetadata => serve_read_file_metadata,
                ReadSymlinkMetadata => serve_read_symlink_metadata
            }
        );
        Ok(Value::Int(result))
    }
}
