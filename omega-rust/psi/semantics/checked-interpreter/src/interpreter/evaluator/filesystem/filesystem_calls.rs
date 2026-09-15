//! Trying and serving one filesystem call: the entry the evaluator takes
//! for every filesystem operation and the dispatch over each call kind.

use crate::interpreter::evaluator::Evaluator;
use crate::interpreter::evaluator::{
    EvalResult, ExpressionHandle, FIND_DATA_OUTPUT_BYTES, FilesystemEvaluationHaltKind,
    FilesystemHostOperation, FilesystemHostResultKind, FilesystemObservationProvider,
    FilesystemOperationAttempt, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    Frame, Halt, MAX_FILESYSTEM_OBSERVATION_EVIDENCE_BYTES, PreparedFilesystemCall,
    PreparedFilesystemPreparation, VIRTUAL_MTIME_SECS, Value, VirtualFd, dirent_record_chunk,
    host_open_flags, synthetic_handle_fd, trap,
};
use crate::{
    FilesystemMetadataObservationKind, FilesystemObservedByteRegionKind,
    FilesystemReturnedPathCompleteness, FilesystemReturnedPathKind,
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
        let replay_expected = self.expected_filesystem_replay_attempt(attempt_index, operation)?;
        let execute_replayed_attempt = self
            .filesystem_replay
            .as_ref()
            .is_some_and(|replay| replay.executes_replay_attempt(attempt_index));
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
                            })
                            .and_then(|()| {
                                self.reject_rooted_final_path_result(attempt_index, &call)
                            }) {
                            Err(halt) => Err(halt),
                            Ok(()) => {
                                match self
                                    .reserve_prepared_live_filesystem_handle(&logical_handle_plan)
                                {
                                    Err(halt) => Err(halt),
                                    Ok(live_handle_lease) => {
                                        let served = match replay_expected.as_ref() {
                                            Some(expected) if execute_replayed_attempt => self
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
                                        let completed = self.complete_mutable_observations(
                                            attempt_index,
                                            &mutable_plan,
                                        );
                                        match (served, completed) {
                                            (Ok(value), Ok(())) => {
                                                Ok((value, logical_handle_plan, live_handle_lease))
                                            }
                                            (Err(halt), Ok(())) => Err(halt),
                                            (_, Err(halt)) => Err(halt),
                                        }
                                    }
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
                if let Some(expected) = replay_expected.as_ref()
                    && &self.filesystem_operation_attempts[attempt_index] != expected
                {
                    self.filesystem_operation_attempts[attempt_index].outcome =
                        Some(FilesystemOperationAttemptOutcome::EvaluationHalted(
                            FilesystemEvaluationHaltKind::Trap,
                        ));
                    return trap(format!("filesystem replay event {attempt_index} changed"));
                }
                // Sealed-file custody follows only fully observed attempts: a
                // replay mismatch or an observation failure must not mint or
                // retire writers.
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

    pub(in crate::interpreter) fn finish_filesystem_replay(&self) -> EvalResult<()> {
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
        let output_directories = replay.output_directories();
        let output_hard_links = replay.output_hard_links();
        let output_symlinks = replay.output_symlinks();
        if outputs.is_empty()
            && output_directories.is_empty()
            && output_hard_links.is_empty()
            && output_symlinks.is_empty()
        {
            if !self.build_included_sources.is_empty() {
                return trap("source-only filesystem replay observed generated-source handoff");
            }
            if !self.virtual_files.is_empty()
                || !self.virtual_fds.is_empty()
                || !self.virtual_dirs.is_empty()
                || !self.virtual_finds.is_empty()
                || !self.virtual_perms.is_empty()
                || !self.virtual_symlinks.is_empty()
                || !self.virtual_times.is_empty()
                || !self.virtual_flocks.is_empty()
            {
                return trap(
                    "filesystem replay with no final Output entries changed its namespace",
                );
            }
            return Ok(());
        }
        if self.build_included_sources.as_slice() != replay.expected_included_sources() {
            return trap("filesystem replay generated-source handoff sequence changed");
        }
        let mut expected_files = std::collections::BTreeMap::new();
        let mut expected_permissions = std::collections::BTreeMap::new();
        let mut expected_times = std::collections::BTreeMap::new();
        for output in outputs {
            let mut expected_path = format!("/root/{}", output.output_root().get()).into_bytes();
            expected_path.push(b'/');
            expected_path.extend_from_slice(output.output_relative_path());
            let bytes = output.replayed_bytes().map_err(Halt::Resource)?;
            if expected_files
                .insert(expected_path.clone(), bytes)
                .is_some()
            {
                return trap("filesystem replay contains duplicate Output paths");
            }
            if let Some(mode) = output.replayed_file_permissions() {
                expected_permissions.insert(expected_path.clone(), mode);
            }
            if let Some(modification_time) = output.replayed_file_modification_time() {
                expected_times.insert(expected_path, modification_time);
            }
        }
        for hard_link in output_hard_links {
            let mut existing_path = format!("/root/{}", hard_link.output_root().get()).into_bytes();
            existing_path.push(b'/');
            existing_path.extend_from_slice(hard_link.existing_relative_path());
            let mut output_path = format!("/root/{}", hard_link.output_root().get()).into_bytes();
            output_path.push(b'/');
            output_path.extend_from_slice(hard_link.output_relative_path());
            let bytes = expected_files
                .get(&existing_path)
                .ok_or_else(|| {
                    Halt::Trap(
                        "filesystem replay hard link has no existing regular-file name".to_owned(),
                    )
                })?
                .clone();
            if expected_files.insert(output_path.clone(), bytes).is_some() {
                return trap("filesystem replay contains duplicate Output paths");
            }
            if let Some(mode) = expected_permissions.get(&existing_path).copied() {
                expected_permissions.insert(output_path.clone(), mode);
            }
            if let Some(modification_time) = expected_times.get(&existing_path).copied() {
                expected_times.insert(output_path, modification_time);
            }
        }
        let expected_directories = output_directories
            .into_iter()
            .map(|directory| {
                let mut path = format!("/root/{}", directory.output_root().get()).into_bytes();
                path.push(b'/');
                path.extend_from_slice(directory.output_relative_path());
                path
            })
            .collect::<std::collections::BTreeSet<_>>();
        let expected_symlinks = output_symlinks
            .into_iter()
            .map(|symlink| {
                let mut path = format!("/root/{}", symlink.output_root().get()).into_bytes();
                path.push(b'/');
                path.extend_from_slice(symlink.output_relative_path());
                (path, symlink.target_spelling().to_vec())
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        if self.virtual_files != expected_files
            || !self.virtual_fds.is_empty()
            || self.virtual_dirs != expected_directories
            || !self.virtual_finds.is_empty()
            || self.virtual_perms != expected_permissions
            || self.virtual_symlinks != expected_symlinks
            || self.virtual_times != expected_times
            || !self.virtual_flocks.is_empty()
        {
            return trap("filesystem replay output namespace changed");
        }
        Ok(())
    }

    /// Drive a value-returning `FilesystemHost` operation against the selected
    /// filesystem provider. Canonical preparation has already evaluated every
    /// authored operand exactly once and validated every mutable output place.
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
                if self.virtual_fds.contains_key(&fd) {
                    0
                } else {
                    self.virtual_errno = 9; // EBADF
                    -1
                }
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
                // taken. The hermetic FS has no inodes, so this snapshots the
                // modeled file state. A later mutation still will not propagate
                // between names, but the snapshot is exact when neither name is
                // subsequently mutated.
                if self.virtual_files.contains_key(&link)
                    || self.virtual_dirs.contains(&link)
                    || self.virtual_symlinks.contains_key(&link)
                {
                    self.virtual_errno = 17; // EEXIST
                    -1
                } else if self.virtual_hard_link_file(&original, link) {
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
                // snapshot model as `hard_link` above. virtual_errno is
                // also the provider's Win32 last-error slot for GetLastError.
                if self.virtual_files.contains_key(&link)
                    || self.virtual_dirs.contains(&link)
                    || self.virtual_symlinks.contains_key(&link)
                {
                    self.virtual_errno = 183; // ERROR_ALREADY_EXISTS
                    0
                } else if self.virtual_hard_link_file(&existing, link) {
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
                        let records = self.build_dirent_records(&path)?;
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
                    .map(|dir_path| self.build_find_entries(dir_path))
                    .transpose()?;
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
}
