/*
    Filesystem access configuration, metadata observations, operation-attempt
    accounting, call preparation, and the hermetic virtual filesystem model.

    We prepare filesystem operands once, track logical handle lifetimes and
    rooted observation evidence, and serve hermetic virtual filesystem calls
    when no physical host provider is selected.
*/

pub(crate) mod filesystem_access {
    use filesystem_authority::FsGrants;

    use crate::FilesystemSponsor;

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct CapturedFilesystemRoot {
        pub identity: crate::FilesystemGrantRootIdentity,
        pub snapshot: source::SourceSnapshot,
    }

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub enum FilesystemAccess {
        #[default]
        Virtual,
        RealUnscoped,
        RealScoped(FsGrants),
        RealScopedSponsored {
            grants: FsGrants,
            sponsor: FilesystemSponsor,
        },
        CapturedScoped {
            inputs: Vec<CapturedFilesystemRoot>,
            grants: FsGrants,
            sponsor: Option<FilesystemSponsor>,
        },
    }
}

pub(crate) mod filesystem_observations {
    use crate::FilesystemMetadataField;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FilesystemMetadataObservationKind {
        FollowedPath,
        OpenDescriptor,
        UnfollowedFinalPath,
    }

    pub const FILESYSTEM_METADATA_API_CARRIER_BYTES: usize = 144;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FilesystemMetadataObservation {
        pub(crate) output_operand_ordinal: u8,
        pub(crate) kind: FilesystemMetadataObservationKind,
        pub(crate) device: u64,
        pub(crate) mode: u32,
        pub(crate) link_count: u64,
        pub(crate) inode: u64,
        pub(crate) user: u32,
        pub(crate) group: u32,
        pub(crate) referenced_device: u64,
        pub(crate) access_time: i64,
        pub(crate) modification_time: i64,
        pub(crate) change_time: i64,
        pub(crate) birth_time: i64,
        pub(crate) size: i64,
        pub(crate) blocks_512: u64,
        pub(crate) preferred_block_size: u64,
    }

    impl FilesystemMetadataObservation {
        pub(crate) const fn new(
            output_operand_ordinal: u8,
            kind: FilesystemMetadataObservationKind,
            mode: u32,
            size: i64,
            modification_time: i64,
        ) -> Self {
            Self {
                output_operand_ordinal,
                kind,
                device: 16_777_220,
                mode,
                link_count: 1,
                inode: 1_000_000,
                user: 501,
                group: 20,
                referenced_device: 0,
                access_time: 1_000_000_100,
                modification_time,
                change_time: 1_000_000_050,
                birth_time: 999_999_900,
                size,
                blocks_512: 8,
                preferred_block_size: 4096,
            }
        }

        pub const fn output_operand_ordinal(self) -> u8 {
            self.output_operand_ordinal
        }
        pub const fn kind(self) -> FilesystemMetadataObservationKind {
            self.kind
        }
        pub const fn device(self) -> u64 {
            self.device
        }
        pub const fn mode(self) -> u32 {
            self.mode
        }
        pub const fn link_count(self) -> u64 {
            self.link_count
        }
        pub const fn inode(self) -> u64 {
            self.inode
        }
        pub const fn user(self) -> u32 {
            self.user
        }
        pub const fn group(self) -> u32 {
            self.group
        }
        pub const fn referenced_device(self) -> u64 {
            self.referenced_device
        }
        pub const fn access_time(self) -> i64 {
            self.access_time
        }
        pub const fn modification_time(self) -> i64 {
            self.modification_time
        }
        pub const fn change_time(self) -> i64 {
            self.change_time
        }
        pub const fn birth_time(self) -> i64 {
            self.birth_time
        }
        pub const fn size(self) -> i64 {
            self.size
        }
        pub const fn blocks_512(self) -> u64 {
            self.blocks_512
        }
        pub const fn preferred_block_size(self) -> u64 {
            self.preferred_block_size
        }

        pub(crate) const fn unsigned_field(self, field: FilesystemMetadataField) -> Option<u64> {
            match field {
                FilesystemMetadataField::Device => Some(self.device),
                FilesystemMetadataField::Mode => Some(self.mode as u64),
                FilesystemMetadataField::LinkCount => Some(self.link_count),
                FilesystemMetadataField::Inode => Some(self.inode),
                FilesystemMetadataField::User => Some(self.user as u64),
                FilesystemMetadataField::Group => Some(self.group as u64),
                FilesystemMetadataField::ReferencedDevice => Some(self.referenced_device),
                FilesystemMetadataField::Blocks512 => Some(self.blocks_512),
                FilesystemMetadataField::PreferredBlockSize => Some(self.preferred_block_size),
                FilesystemMetadataField::AccessTime
                | FilesystemMetadataField::ModificationTime
                | FilesystemMetadataField::ChangeTime
                | FilesystemMetadataField::BirthTime
                | FilesystemMetadataField::Size => None,
            }
        }

        pub(crate) const fn signed_field(self, field: FilesystemMetadataField) -> Option<i64> {
            match field {
                FilesystemMetadataField::AccessTime => Some(self.access_time),
                FilesystemMetadataField::ModificationTime => Some(self.modification_time),
                FilesystemMetadataField::ChangeTime => Some(self.change_time),
                FilesystemMetadataField::BirthTime => Some(self.birth_time),
                FilesystemMetadataField::Size => Some(self.size),
                _ => None,
            }
        }
    }
}

pub(crate) mod filesystem_operation_attempts {
    use filesystem_authority::{
        FilesystemAuthorizedPath, FilesystemGrantRefusal, FilesystemLogicalHandleIdentity,
        FilesystemLogicalHandleInput, FilesystemLogicalHandleOutput, FilesystemObservationProvider,
        FilesystemOperationResult, FilesystemRootedPathOperandResolution,
    };

    pub use filesystem_authority::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FilesystemEvaluationHaltKind {
        Exit,
        Unsupported,
        Trap,
        ResourceExhausted,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FilesystemOperationAttemptOutcome {
        Returned {
            result: FilesystemOperationResult,
            post_error: i32,
        },
        EvaluationHalted(FilesystemEvaluationHaltKind),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FilesystemOperationAttempt {
        pub(crate) operation_tag: u16,
        pub(crate) provider: FilesystemObservationProvider,
        pub(crate) outcome: Option<FilesystemOperationAttemptOutcome>,
        pub(crate) rooted_path_operand_resolutions: Vec<FilesystemRootedPathOperandResolution>,
        pub(crate) authorized_paths: Vec<FilesystemAuthorizedPath>,
        pub(crate) logical_handle_inputs: Vec<FilesystemLogicalHandleInput>,
        pub(crate) logical_handle_output: Option<FilesystemLogicalHandleOutput>,
        pub(crate) retired_logical_handles: Vec<FilesystemLogicalHandleIdentity>,
        pub(crate) grant_refusals: Vec<FilesystemGrantRefusal>,
    }

    impl FilesystemOperationAttempt {
        pub(crate) const fn pending(
            operation_tag: u16,
            provider: FilesystemObservationProvider,
        ) -> Self {
            Self {
                operation_tag,
                provider,
                outcome: None,
                rooted_path_operand_resolutions: Vec::new(),
                authorized_paths: Vec::new(),
                logical_handle_inputs: Vec::new(),
                logical_handle_output: None,
                retired_logical_handles: Vec::new(),
                grant_refusals: Vec::new(),
            }
        }

        pub const fn operation_tag(&self) -> u16 {
            self.operation_tag
        }

        pub const fn provider(&self) -> FilesystemObservationProvider {
            self.provider
        }

        pub const fn outcome(&self) -> Option<FilesystemOperationAttemptOutcome> {
            self.outcome
        }

        pub const fn result(&self) -> Option<FilesystemOperationResult> {
            match self.outcome {
                Some(FilesystemOperationAttemptOutcome::Returned { result, .. }) => Some(result),
                _ => None,
            }
        }

        pub const fn post_error(&self) -> Option<i32> {
            match self.outcome {
                Some(FilesystemOperationAttemptOutcome::Returned { post_error, .. }) => {
                    Some(post_error)
                }
                _ => None,
            }
        }

        pub fn rooted_path_operand_resolutions(&self) -> &[FilesystemRootedPathOperandResolution] {
            &self.rooted_path_operand_resolutions
        }

        pub fn grant_refusals(&self) -> &[FilesystemGrantRefusal] {
            &self.grant_refusals
        }

        pub fn authorized_paths(&self) -> &[FilesystemAuthorizedPath] {
            &self.authorized_paths
        }

        pub fn logical_handle_inputs(&self) -> &[FilesystemLogicalHandleInput] {
            &self.logical_handle_inputs
        }

        pub const fn logical_handle_output(&self) -> Option<FilesystemLogicalHandleOutput> {
            self.logical_handle_output
        }

        pub fn retired_logical_handles(&self) -> &[FilesystemLogicalHandleIdentity] {
            &self.retired_logical_handles
        }
    }
}

pub(crate) mod evaluator_filesystem {

    use crate::interpreter::evaluator::filesystem::preparation::PreparedByteOutput;
    use crate::interpreter::evaluator::{EvalResult, Evaluator};

    impl<'program> Evaluator<'program> {
        pub(crate) fn pop_virtual_find(
            &mut self,
            handle: i64,
            data: &PreparedByteOutput,
        ) -> EvalResult<i64> {
            Ok(
                match self
                    .virtual_finds
                    .get_mut(&handle)
                    .and_then(std::collections::VecDeque::pop_front)
                {
                    Some((name, is_dir)) => {
                        self.write_find_data(data, &name, is_dir)?;
                        1
                    }
                    None => 0,
                },
            )
        }

        pub(crate) fn close_virtual_find(&mut self, handle: i64) -> i64 {
            if self.virtual_finds.remove(&handle).is_some() {
                1
            } else {
                0
            }
        }
    }

    pub(crate) mod filesystem_calls {
        use crate::interpreter::evaluator::Evaluator;
        use crate::interpreter::evaluator::{
            EvalResult, FilesystemEvaluationHaltKind, FilesystemHostOperation,
            FilesystemHostResultKind, FilesystemObservationProvider, FilesystemOperationAttempt,
            FilesystemOperationAttemptOutcome, FilesystemOperationResult, Halt,
            PreparedFilesystemCall, Value,
        };

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

        impl<'program> Evaluator<'program> {
            pub(crate) fn evaluate_filesystem_operation(
                &mut self,
                operation: FilesystemHostOperation,
                prepare: impl FnOnce(&mut Self) -> EvalResult<PreparedFilesystemCall>,
            ) -> EvalResult<Value> {
                self.charge_filesystem_operation_attempt()?;
                let attempt_index = self.filesystem_operation_attempts.len();
                let provider = FilesystemObservationProvider::Virtual;
                self.filesystem_operation_attempts
                    .push(FilesystemOperationAttempt::pending(
                        operation.operation_tag(),
                        provider,
                    ));
                self.filesystem_operation_attempt_stack.push(attempt_index);
                let mut outcome = (|| {
                    let call = prepare(self)?;
                    let logical_handle_plan = call.logical_handle_plan();
                    call.validate_output_carriers()?;
                    self.validate_incremental_logical_handle_inputs(
                        attempt_index,
                        &logical_handle_plan,
                    )?;
                    self.reject_cross_domain_logical_handle_inputs(&logical_handle_plan)?;
                    self.reject_rooted_final_path_result(attempt_index, &call)?;
                    let live_handle_lease =
                        self.reserve_prepared_live_filesystem_handle(&logical_handle_plan)?;
                    let value = self.serve_filesystem_call(call)?;
                    Ok((value, logical_handle_plan, live_handle_lease))
                })();
                if let Some(message) = self.filesystem_observation_resource_halt.take() {
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
                                Halt::Trap(_)
                                | Halt::Crash(_, _)
                                | Halt::StaticInputUnavailable(_) => {
                                    FilesystemEvaluationHaltKind::Trap
                                }
                                Halt::Resource(_) => {
                                    FilesystemEvaluationHaltKind::ResourceExhausted
                                }
                            };
                            self.filesystem_operation_attempts[attempt_index].outcome =
                                Some(FilesystemOperationAttemptOutcome::EvaluationHalted(kind));
                            return Err(halt);
                        }
                        let post_error = self.virtual_errno;
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
                        self.note_completed_build_output_attempt(attempt_index);
                        Ok(value)
                    }
                    Err(halt) => {
                        let kind = match &halt {
                            Halt::Exit(_) => FilesystemEvaluationHaltKind::Exit,
                            Halt::Unsupported(_) => FilesystemEvaluationHaltKind::Unsupported,
                            Halt::Trap(_) | Halt::Crash(_, _) | Halt::StaticInputUnavailable(_) => {
                                FilesystemEvaluationHaltKind::Trap
                            }
                            Halt::Resource(_) => FilesystemEvaluationHaltKind::ResourceExhausted,
                        };
                        self.filesystem_operation_attempts[attempt_index].outcome =
                            Some(FilesystemOperationAttemptOutcome::EvaluationHalted(kind));
                        Err(halt)
                    }
                }
            }

            pub(crate) fn serve_filesystem_call(
                &mut self,
                call: PreparedFilesystemCall,
            ) -> EvalResult<Value> {
                let result: i64 = prepared_filesystem_call_dispatch!(
                    self, call;
                    literal {
                        PreparedFilesystemCall::Create { path } => {
                            self.virtual_open(path.into_native()?, true, true) as i64
                        }
                        PreparedFilesystemCall::Open { path, flags } => {
                            self.virtual_open_flags(path.into_native()?, flags) as i64
                        }
                        PreparedFilesystemCall::GetLastError => { i64::from(self.virtual_errno) }
                        PreparedFilesystemCall::Errno => {
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
                        Remove | RemoveName => serve_remove,
                        Seek => serve_seek,
                        SetLen => serve_set_len,
                        SetFilePermissions => serve_set_file_permissions,
                        SetFileTimes => serve_set_file_times,
                        Sync | SyncData => serve_sync,
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

        pub(crate) mod descriptor_calls {
            use crate::FilesystemMetadataObservationKind;
            use crate::interpreter::evaluator::{
                EvalResult, PreparedFilesystemCall, VIRTUAL_MTIME_SECS, VirtualFd,
                synthetic_handle_fd,
            };

            impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
                fn virtual_descriptor_path(&self, fd: i32) -> Option<Vec<u8>> {
                    self.virtual_fds
                        .get(&fd)
                        .map(|descriptor| descriptor.path.clone())
                }

                pub(crate) fn serve_read(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Read { fd, buffer, count } = call else {
                        unreachable!("dispatched serve_read")
                    };
                    Ok({
                        match self.virtual_read_n(fd, count.host) {
                            Some(bytes) => {
                                let n = bytes.len() as i64;
                                buffer.write(&bytes)?;
                                n
                            }
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_write(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Write { fd, bytes } = call else {
                        unreachable!("dispatched serve_write")
                    };
                    Ok({
                        match self.virtual_write(fd, &bytes) {
                            Some(count) => count as i64,
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_read_at(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ReadAt {
                        fd,
                        buffer,
                        count,
                        offset,
                    } = call
                    else {
                        unreachable!("dispatched serve_read_at")
                    };
                    Ok({
                        match self.virtual_read_at(fd, offset, count.host) {
                            Some(bytes) => {
                                let n = bytes.len() as i64;
                                buffer.write(&bytes)?;
                                n
                            }
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_write_at(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::WriteAt { fd, bytes, offset } = call else {
                        unreachable!("dispatched serve_write_at")
                    };
                    Ok({
                        match self.virtual_write_at(fd, offset, &bytes) {
                            Some(count) => count as i64,
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_close(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Close { fd } = call else {
                        unreachable!("dispatched serve_close")
                    };
                    Ok({
                        if self.virtual_fds.remove(&fd).is_some() {
                            self.virtual_flocks.retain(|_, owner| *owner != fd);
                            0
                        } else {
                            self.errno_result(9, -1)
                        }
                    })
                }

                pub(crate) fn serve_close_handle(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::CloseHandle { handle } = call else {
                        unreachable!("dispatched serve_close_handle")
                    };
                    Ok({
                        match synthetic_handle_fd(handle) {
                            Some(handle) if self.virtual_fds.remove(&handle).is_some() => {
                                self.virtual_flocks.retain(|_, owner| *owner != handle);
                                1
                            }
                            _ => self.errno_result(6, 0),
                        }
                    })
                }

                pub(crate) fn serve_duplicate(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Duplicate { fd } = call else {
                        unreachable!("dispatched serve_duplicate")
                    };
                    Ok({
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
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_lock_file(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::LockFile { fd, operation } = call else {
                        unreachable!("dispatched serve_lock_file")
                    };
                    Ok({
                        let path = self.virtual_descriptor_path(fd);
                        match path {
                            None => self.errno_result(9, -1),
                            Some(path) if operation & 8 != 0 => {
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
                                    self.errno_result(35, -1)
                                } else {
                                    self.virtual_flocks.insert(path, fd);
                                    0
                                }
                            }
                        }
                    })
                }

                pub(crate) fn serve_lock_file_ex(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::LockFileEx { handle, flags } = call else {
                        unreachable!("dispatched serve_lock_file_ex")
                    };
                    Ok({
                        let Some(fd) = synthetic_handle_fd(handle) else {
                            self.virtual_errno = 6;
                            return Ok(0);
                        };
                        let flags = flags as i32;
                        let path = self.virtual_descriptor_path(fd);
                        match path {
                            None => self.errno_result(6, 0),
                            Some(path) => {
                                let held_by_other = matches!(
                                    self.virtual_flocks.get(&path),
                                    Some(owner) if *owner != fd
                                );
                                if held_by_other && flags & 1 != 0 {
                                    self.errno_result(33, 0)
                                } else {
                                    self.virtual_flocks.insert(path, fd);
                                    1
                                }
                            }
                        }
                    })
                }

                pub(crate) fn serve_unlock_file(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::UnlockFile { handle } = call else {
                        unreachable!("dispatched serve_unlock_file")
                    };
                    Ok({
                        let Some(fd) = synthetic_handle_fd(handle) else {
                            self.virtual_errno = 6;
                            return Ok(0);
                        };
                        let path = self.virtual_descriptor_path(fd);
                        match path {
                            None => self.errno_result(6, 0),
                            Some(path) if self.virtual_flocks.get(&path) == Some(&fd) => {
                                self.virtual_flocks.remove(&path);
                                1
                            }
                            Some(_) => self.errno_result(158, 0),
                        }
                    })
                }

                pub(crate) fn serve_seek(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Seek { fd, offset, whence } = call else {
                        unreachable!("dispatched serve_seek")
                    };
                    Ok({
                        match self.virtual_seek(fd, offset, whence) {
                            Some(position) => position,
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_set_len(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::SetLen { fd, length } = call else {
                        unreachable!("dispatched serve_set_len")
                    };
                    Ok({
                        let rc = self.virtual_set_len(fd, length);
                        if rc < 0 {
                            self.virtual_errno = 9;
                        }
                        rc
                    })
                }

                pub(crate) fn serve_set_file_permissions(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::SetFilePermissions { fd, mode } = call else {
                        unreachable!("dispatched serve_set_file_permissions")
                    };
                    Ok({
                        match self.virtual_fds.get(&fd) {
                            Some(descriptor) => {
                                let path = descriptor.path.clone();
                                self.virtual_perms.insert(path, mode);
                                0
                            }
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_set_file_times(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::SetFileTimes { fd, times } = call else {
                        unreachable!("dispatched serve_set_file_times")
                    };
                    Ok({
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
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_sync(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let (PreparedFilesystemCall::Sync { fd }
                    | PreparedFilesystemCall::SyncData { fd }) = call
                    else {
                        unreachable!("dispatched serve_sync")
                    };
                    Ok({
                        if self.virtual_fds.contains_key(&fd) {
                            0
                        } else {
                            self.errno_result(9, -1)
                        }
                    })
                }

                pub(crate) fn serve_change_file_owner(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ChangeFileOwner { fd, uid, gid } = call else {
                        unreachable!("dispatched serve_change_file_owner")
                    };
                    Ok({
                        if self.virtual_fds.contains_key(&fd) {
                            self.virtual_chown_result(uid, gid)
                        } else {
                            self.errno_result(9, -1)
                        }
                    })
                }

                pub(crate) fn serve_get_osf_handle(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::GetOsfHandle { fd } = call else {
                        unreachable!("dispatched serve_get_osf_handle")
                    };
                    Ok({
                        if self.virtual_fds.contains_key(&fd) {
                            i64::from(fd)
                        } else {
                            -2
                        }
                    })
                }

                pub(crate) fn serve_final_path_name_by_handle(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::FinalPathNameByHandle {
                        handle,
                        buffer,
                        capacity,
                    } = call
                    else {
                        unreachable!("dispatched serve_final_path_name_by_handle")
                    };
                    Ok({
                        let path = synthetic_handle_fd(handle)
                            .and_then(|handle| self.virtual_fds.get(&handle))
                            .map(|descriptor| descriptor.path.clone());
                        match path {
                            Some(path) => {
                                if path.len() < capacity.host {
                                    let mut bytes = path.clone();
                                    bytes.push(0);
                                    buffer.write(&bytes)?;
                                    path.len() as i64
                                } else {
                                    (path.len() + 1) as i64
                                }
                            }
                            None => self.errno_result(6, 0),
                        }
                    })
                }

                pub(crate) fn serve_set_file_time(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::SetFileTime {
                        handle,
                        last_write: write_ft,
                    } = call
                    else {
                        unreachable!("dispatched serve_set_file_time")
                    };
                    Ok({
                        match synthetic_handle_fd(handle)
                            .and_then(|handle| self.virtual_fds.get(&handle))
                        {
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
                            None => self.errno_result(6, 0),
                        }
                    })
                }

                pub(crate) fn serve_read_file_metadata(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ReadFileMetadata { fd, buffer } = call else {
                        unreachable!("dispatched serve_read_file_metadata")
                    };
                    Ok({
                        let path = self.virtual_descriptor_path(fd);
                        let meta = path.and_then(|path| {
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
                            None => self.errno_result(9, -1),
                        }
                    })
                }
            }
        }

        pub(crate) mod directory_calls {
            use crate::interpreter::evaluator::{
                EvalResult, PreparedFilesystemCall, dirent_record_chunk,
            };

            impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
                pub(crate) fn serve_read_dir(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ReadDir {
                        fd,
                        buffer,
                        count,
                        position,
                    } = call
                    else {
                        unreachable!("dispatched serve_read_dir")
                    };
                    Ok({
                        let dir_path = self
                            .virtual_fds
                            .get(&fd)
                            .filter(|descriptor| descriptor.is_dir)
                            .map(|descriptor| descriptor.path.clone());
                        match dir_path {
                            None => {
                                self.virtual_errno = if self.virtual_fds.contains_key(&fd) {
                                    20
                                } else {
                                    9
                                };
                                -1
                            }
                            Some(path) => {
                                let records = self.build_dirent_records(&path)?;
                                let start = position.initial.max(0) as usize;
                                let (chunk, next_position) =
                                    dirent_record_chunk(&records, start, count.host);
                                if chunk.is_empty() {
                                    0
                                } else {
                                    let n = chunk.len();
                                    buffer.write(chunk)?;
                                    position.write(next_position as i64)?;
                                    n as i64
                                }
                            }
                        }
                    })
                }

                pub(crate) fn serve_find_first(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::FindFirst { pattern, data } = call else {
                        unreachable!("dispatched serve_find_first")
                    };
                    Ok({
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
                                let handle = self.virtual_next_find;
                                self.virtual_next_find += 1;
                                self.virtual_finds.insert(handle, entries);
                                handle
                            }
                            None => self.errno_result(2, -1),
                        }
                    })
                }

                pub(crate) fn serve_find_next(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::FindNext { handle, data } = call else {
                        unreachable!("dispatched serve_find_next")
                    };
                    self.pop_virtual_find(handle, &data)
                }

                pub(crate) fn serve_find_close(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::FindClose { handle } = call else {
                        unreachable!("dispatched serve_find_close")
                    };
                    Ok(self.close_virtual_find(handle))
                }
            }
        }

        pub(crate) mod path_calls {
            use crate::FilesystemMetadataObservationKind;
            use crate::interpreter::evaluator::filesystem::host_open_flags::{o_creat, o_excl};
            use crate::interpreter::evaluator::{
                EvalResult, PreparedFilesystemCall, VIRTUAL_MTIME_SECS,
            };

            impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
                pub(crate) fn serve_open_path_handle(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::OpenPathHandle { path, .. } = call else {
                        unreachable!("dispatched serve_open_path_handle")
                    };
                    let path = path.into_native()?;
                    Ok({
                        let fd = self.virtual_open_flags(path, 0);
                        if fd < 0 {
                            self.virtual_errno = match self.virtual_errno {
                                13 => 5,
                                9 => 6,
                                17 => 183,
                                _ => 2,
                            };
                        }
                        fd as i64
                    })
                }

                pub(crate) fn serve_open_create(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::OpenCreate { path, flags, mode } = call else {
                        unreachable!("dispatched serve_open_create")
                    };
                    let path = path.into_native()?;
                    Ok({
                        let exists = self.virtual_files.contains_key(&path)
                            || self.virtual_dirs.contains(&path)
                            || self.virtual_char_devices.contains(&path);
                        if o_creat(flags) && o_excl(flags) && exists {
                            self.errno_result(17, -1)
                        } else {
                            let created = o_creat(flags) && !exists;
                            let fd = self.virtual_open_flags(path.clone(), flags);
                            if fd >= 0 && created {
                                self.virtual_perms.insert(path, (mode as u32) & 0o777);
                            }
                            fd as i64
                        }
                    })
                }

                pub(crate) fn serve_remove(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let (PreparedFilesystemCall::Remove { path }
                    | PreparedFilesystemCall::RemoveName { path }) = call
                    else {
                        unreachable!("dispatched serve_remove")
                    };
                    let path = path.into_native()?;
                    Ok({
                        if self.virtual_files.remove(&path).is_some() {
                            0
                        } else {
                            self.errno_result(2, -1)
                        }
                    })
                }

                pub(crate) fn serve_create_dir(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let (PreparedFilesystemCall::CreateDir { path }
                    | PreparedFilesystemCall::CreateDirName { name: path }) = call
                    else {
                        unreachable!("dispatched serve_create_dir")
                    };
                    let path = path.into_native()?;
                    Ok({
                        if self.virtual_dirs.insert(path) {
                            0
                        } else {
                            self.errno_result(17, -1)
                        }
                    })
                }

                pub(crate) fn serve_remove_dir(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let (PreparedFilesystemCall::RemoveDir { path }
                    | PreparedFilesystemCall::RemoveDirName { path }) = call
                    else {
                        unreachable!("dispatched serve_remove_dir")
                    };
                    let path = path.into_native()?;
                    Ok({
                        if self.virtual_dirs.remove(&path) {
                            0
                        } else {
                            self.errno_result(2, -1)
                        }
                    })
                }

                pub(crate) fn serve_open_at(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::OpenAt { dirfd, name, flags } = call else {
                        unreachable!("dispatched serve_open_at")
                    };
                    Ok({
                        match self.virtual_at_path(dirfd, &name) {
                            Some(full) => self.virtual_open_flags(full, flags) as i64,
                            None => self.errno_result(9, -1),
                        }
                    })
                }

                pub(crate) fn serve_unlink_at(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::UnlinkAt { dirfd, name, flags } = call else {
                        unreachable!("dispatched serve_unlink_at")
                    };
                    Ok({
                        match self.virtual_at_path(dirfd, &name) {
                            None => self.errno_result(9, -1),
                            Some(full) => {
                                let removed = if (flags & 128) != 0 {
                                    self.virtual_dirs.remove(&full)
                                } else {
                                    self.virtual_files.remove(&full).is_some()
                                };
                                if removed { 0 } else { self.errno_result(2, -1) }
                            }
                        }
                    })
                }

                pub(crate) fn serve_set_permissions(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::SetPermissions { path, mode } = call else {
                        unreachable!("dispatched serve_set_permissions")
                    };
                    let path = path.into_native()?;
                    Ok({
                        if self.virtual_files.contains_key(&path)
                            || self.virtual_dirs.contains(&path)
                        {
                            self.virtual_perms.insert(path, mode);
                            0
                        } else {
                            self.errno_result(2, -1)
                        }
                    })
                }

                pub(crate) fn serve_change_owner(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let (PreparedFilesystemCall::ChangeOwner { path, uid, gid }
                    | PreparedFilesystemCall::ChangeOwnerNoFollow { path, uid, gid }) = call
                    else {
                        unreachable!("dispatched serve_change_owner")
                    };
                    let path = path.into_native()?;
                    Ok({
                        let exists = self.virtual_files.contains_key(&path)
                            || self.virtual_dirs.contains(&path)
                            || self.virtual_symlinks.contains_key(&path);
                        if !exists {
                            self.errno_result(2, -1)
                        } else {
                            self.virtual_chown_result(uid, gid)
                        }
                    })
                }

                pub(crate) fn serve_rename(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Rename { from, to } = call else {
                        unreachable!("dispatched serve_rename")
                    };
                    let from = from.into_native()?;
                    let to = to.into_native()?;
                    Ok({
                        match self.virtual_files.remove(&from) {
                            Some(content) => {
                                self.virtual_files.insert(to, content);
                                0
                            }
                            None => self.errno_result(2, -1),
                        }
                    })
                }

                pub(crate) fn serve_hard_link(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::HardLink { original, link } = call else {
                        unreachable!("dispatched serve_hard_link")
                    };
                    let original = original.into_native()?;
                    let link = link.into_native()?;
                    Ok({
                        if self.virtual_files.contains_key(&link)
                            || self.virtual_dirs.contains(&link)
                            || self.virtual_symlinks.contains_key(&link)
                        {
                            self.errno_result(17, -1)
                        } else if self.virtual_hard_link_file(&original, link) {
                            0
                        } else {
                            self.errno_result(2, -1)
                        }
                    })
                }

                pub(crate) fn serve_create_hard_link(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::CreateHardLink { link, existing } = call else {
                        unreachable!("dispatched serve_create_hard_link")
                    };
                    let link = link.into_native()?;
                    let existing = existing.into_native()?;
                    Ok({
                        if self.virtual_files.contains_key(&link)
                            || self.virtual_dirs.contains(&link)
                            || self.virtual_symlinks.contains_key(&link)
                        {
                            self.errno_result(183, 0)
                        } else if self.virtual_hard_link_file(&existing, link) {
                            1
                        } else {
                            self.errno_result(2, 0)
                        }
                    })
                }

                pub(crate) fn serve_symlink(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Symlink { target, link } = call else {
                        unreachable!("dispatched serve_symlink")
                    };
                    let link = link.into_native()?;
                    Ok({
                        if self.virtual_files.contains_key(&link)
                            || self.virtual_dirs.contains(&link)
                            || self.virtual_symlinks.contains_key(&link)
                        {
                            self.errno_result(17, -1)
                        } else {
                            self.virtual_symlinks.insert(link, target);
                            0
                        }
                    })
                }

                pub(crate) fn serve_read_link(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ReadLink {
                        path,
                        buffer,
                        count,
                    } = call
                    else {
                        unreachable!("dispatched serve_read_link")
                    };
                    let path = path.into_native()?;
                    Ok({
                        match self.virtual_symlinks.get(&path).cloned() {
                            Some(target) => {
                                let n = target.len().min(count.host);
                                buffer.write(&target[..n])?;
                                n as i64
                            }
                            None => self.errno_result(2, -1),
                        }
                    })
                }

                pub(crate) fn serve_canonicalize(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::Canonicalize { path, buffer } = call else {
                        unreachable!("dispatched serve_canonicalize")
                    };
                    let path = path.into_native()?;
                    Ok({
                        let resolved = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
                        let exists = self.virtual_files.contains_key(&resolved)
                            || self.virtual_dirs.contains(&resolved);
                        if exists {
                            let mut bytes = resolved.clone();
                            bytes.push(0);
                            buffer.write(&bytes)?;
                            1
                        } else {
                            self.errno_result(2, 0)
                        }
                    })
                }

                pub(crate) fn serve_read_metadata(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ReadMetadata { path, buffer } = call else {
                        unreachable!("dispatched serve_read_metadata")
                    };
                    let path = path.into_native()?;
                    Ok({
                        let chmod_perm = self
                            .virtual_perms
                            .get(&path)
                            .map(|mode| (*mode as u16) & 0o7777);
                        let meta = if self.virtual_char_devices.contains(&path) {
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
                            None => self.errno_result(2, -1),
                        }
                    })
                }

                pub(crate) fn serve_read_symlink_metadata(
                    &mut self,
                    call: PreparedFilesystemCall,
                ) -> EvalResult<i64> {
                    let PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer } = call else {
                        unreachable!("dispatched serve_read_symlink_metadata")
                    };
                    let path = path.into_native()?;
                    Ok({
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
                            None => self.errno_result(2, -1),
                        }
                    })
                }
            }
        }
    }

    pub(crate) mod host_open_flags {
        pub use filesystem_authority::open_flags::*;
    }

    pub(crate) mod host_operation {
        pub(crate) use filesystem_authority::{FilesystemHostOperation, FilesystemHostResultKind};
    }

    pub(crate) mod logical_handle_store {
        use crate::{
            FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInputResolution,
            FilesystemLogicalHandleKind,
        };
        use std::collections::{BTreeMap, BTreeSet};

        #[derive(Debug)]
        pub(crate) enum FilesystemLogicalHandleError {
            IdentityExhausted,
            LiveProviderTokenCollision {
                kind: FilesystemLogicalHandleKind,
                raw: i64,
            },
            BorrowSourceMismatch {
                raw: i64,
            },
        }

        #[derive(Debug)]
        pub(crate) struct FilesystemLogicalHandles {
            next_identity: u64,
            descriptors: BTreeMap<i64, FilesystemLogicalHandleIdentity>,
            native_handles: BTreeMap<i64, FilesystemLogicalHandleIdentity>,
            find_handles: BTreeMap<i64, FilesystemLogicalHandleIdentity>,
            borrowed_native_sources:
                BTreeMap<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleIdentity>,
        }

        impl Default for FilesystemLogicalHandles {
            fn default() -> Self {
                Self {
                    next_identity: 1,
                    descriptors: BTreeMap::new(),
                    native_handles: BTreeMap::new(),
                    find_handles: BTreeMap::new(),
                    borrowed_native_sources: BTreeMap::new(),
                }
            }
        }

        impl FilesystemLogicalHandles {
            pub(crate) fn resolve(
                &self,
                kind: FilesystemLogicalHandleKind,
                raw: i64,
                null_allowed: bool,
            ) -> FilesystemLogicalHandleInputResolution {
                if null_allowed && raw == 0 {
                    return FilesystemLogicalHandleInputResolution::Null;
                }
                self.map(kind)
                    .get(&raw)
                    .copied()
                    .map(FilesystemLogicalHandleInputResolution::Resolved)
                    .unwrap_or(FilesystemLogicalHandleInputResolution::Unknown)
            }

            pub(crate) fn conflicts_with_live_domain(
                &self,
                expected: FilesystemLogicalHandleKind,
                raw: i64,
                null_allowed: bool,
            ) -> bool {
                if null_allowed && raw == 0 || self.map(expected).contains_key(&raw) {
                    return false;
                }
                [
                    FilesystemLogicalHandleKind::Descriptor,
                    FilesystemLogicalHandleKind::Native,
                    FilesystemLogicalHandleKind::Find,
                ]
                .into_iter()
                .any(|kind| kind != expected && self.map(kind).contains_key(&raw))
            }

            pub(crate) fn create(
                &mut self,
                kind: FilesystemLogicalHandleKind,
                raw: i64,
            ) -> Result<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleError> {
                if self.map(kind).contains_key(&raw) {
                    return Err(FilesystemLogicalHandleError::LiveProviderTokenCollision {
                        kind,
                        raw,
                    });
                }
                let identity = self.allocate()?;
                self.map_mut(kind).insert(raw, identity);
                Ok(identity)
            }

            pub(crate) fn borrow_native(
                &mut self,
                raw: i64,
                source: FilesystemLogicalHandleIdentity,
            ) -> Result<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleError> {
                if let Some(existing) = self.native_handles.get(&raw).copied() {
                    if self.borrowed_native_sources.get(&existing) == Some(&source) {
                        return Ok(existing);
                    }
                    return Err(FilesystemLogicalHandleError::BorrowSourceMismatch { raw });
                }
                let identity = self.allocate()?;
                self.native_handles.insert(raw, identity);
                self.borrowed_native_sources.insert(identity, source);
                Ok(identity)
            }

            pub(crate) fn retire(
                &mut self,
                kind: FilesystemLogicalHandleKind,
                identity: FilesystemLogicalHandleIdentity,
            ) -> Vec<FilesystemLogicalHandleIdentity> {
                let mut retired = BTreeSet::new();
                match kind {
                    FilesystemLogicalHandleKind::Descriptor => {
                        self.retire_descriptor(identity, &mut retired);
                    }
                    FilesystemLogicalHandleKind::Native => {
                        let borrowed_source = self.borrowed_native_sources.remove(&identity);
                        remove_identity(&mut self.native_handles, identity);
                        retired.insert(identity);
                        if let Some(source) = borrowed_source {
                            self.retire_descriptor(source, &mut retired);
                        }
                    }
                    FilesystemLogicalHandleKind::Find => {
                        remove_identity(&mut self.find_handles, identity);
                        retired.insert(identity);
                    }
                }
                retired.into_iter().collect()
            }

            fn retire_descriptor(
                &mut self,
                identity: FilesystemLogicalHandleIdentity,
                retired: &mut BTreeSet<FilesystemLogicalHandleIdentity>,
            ) {
                remove_identity(&mut self.descriptors, identity);
                retired.insert(identity);
                let aliases = self
                    .borrowed_native_sources
                    .iter()
                    .filter_map(|(alias, source)| (*source == identity).then_some(*alias))
                    .collect::<Vec<_>>();
                for alias in aliases {
                    self.borrowed_native_sources.remove(&alias);
                    remove_identity(&mut self.native_handles, alias);
                    retired.insert(alias);
                }
            }

            fn allocate(
                &mut self,
            ) -> Result<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleError> {
                let identity = FilesystemLogicalHandleIdentity::new(self.next_identity)
                    .ok_or(FilesystemLogicalHandleError::IdentityExhausted)?;
                self.next_identity = self
                    .next_identity
                    .checked_add(1)
                    .ok_or(FilesystemLogicalHandleError::IdentityExhausted)?;
                Ok(identity)
            }

            fn map(
                &self,
                kind: FilesystemLogicalHandleKind,
            ) -> &BTreeMap<i64, FilesystemLogicalHandleIdentity> {
                match kind {
                    FilesystemLogicalHandleKind::Descriptor => &self.descriptors,
                    FilesystemLogicalHandleKind::Native => &self.native_handles,
                    FilesystemLogicalHandleKind::Find => &self.find_handles,
                }
            }

            fn map_mut(
                &mut self,
                kind: FilesystemLogicalHandleKind,
            ) -> &mut BTreeMap<i64, FilesystemLogicalHandleIdentity> {
                match kind {
                    FilesystemLogicalHandleKind::Descriptor => &mut self.descriptors,
                    FilesystemLogicalHandleKind::Native => &mut self.native_handles,
                    FilesystemLogicalHandleKind::Find => &mut self.find_handles,
                }
            }
        }

        fn remove_identity(
            map: &mut BTreeMap<i64, FilesystemLogicalHandleIdentity>,
            identity: FilesystemLogicalHandleIdentity,
        ) {
            map.retain(|_, candidate| *candidate != identity);
        }
    }

    pub(crate) mod logical_handles {
        use crate::interpreter::evaluator::{
            BuildEvaluationLiveFilesystemHandleLease, BuildEvaluationSponsor, EvalResult,
            Evaluator, FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
            FilesystemLogicalHandleKind, FilesystemLogicalHandleOutput,
            FilesystemLogicalHandleOutputSource, Halt, PreparedFilesystemCall,
            PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
            filesystem::logical_handle_store, trap,
        };

        impl<'program> Evaluator<'program> {
            pub(crate) fn record_prepared_filesystem_logical_handle_input(
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
                    .push(FilesystemLogicalHandleInput::new(
                        operand_ordinal,
                        kind,
                        resolution,
                    ));
            }

            pub(crate) fn validate_incremental_logical_handle_inputs(
                &mut self,
                attempt_index: usize,
                plan: &PreparedFilesystemLogicalHandlePlan,
            ) -> EvalResult<()> {
                let canonical_inputs = plan
                    .inputs
                    .iter()
                    .map(|input| {
                        FilesystemLogicalHandleInput::new(
                            input.operand_ordinal,
                            input.kind,
                            self.filesystem_logical_handles.resolve(
                                input.kind,
                                input.raw,
                                input.null_allowed,
                            ),
                        )
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

            pub(crate) fn reject_cross_domain_logical_handle_inputs(
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

            pub(crate) fn reject_rooted_final_path_result(
                &self,
                attempt_index: usize,
                call: &PreparedFilesystemCall,
            ) -> EvalResult<()> {
                if !self.rooted_build_paths_required
                    || !matches!(call, PreparedFilesystemCall::FinalPathNameByHandle { .. })
                {
                    return Ok(());
                }
                let inputs =
                    &self.filesystem_operation_attempts[attempt_index].logical_handle_inputs;
                if inputs.len() == 1 && inputs[0].is_unknown_native() {
                    return Ok(());
                }
                trap(
                    "package build filesystem operation `final_path_name_by_handle` would expose a host-absolute path",
                )
            }

            pub(crate) fn complete_logical_handle_observations(
                &mut self,
                attempt_index: usize,
                plan: &PreparedFilesystemLogicalHandlePlan,
                result: i64,
                live_handle_lease: &mut Option<BuildEvaluationLiveFilesystemHandleLease>,
            ) -> EvalResult<()> {
                if plan
                    .input_success
                    .is_some_and(|success| success.accepts(result))
                    && self.filesystem_operation_attempts[attempt_index]
                        .logical_handle_inputs
                        .iter()
                        .any(|input| {
                            matches!(
                                input.resolution(),
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
                            Some(FilesystemLogicalHandleOutput::new(kind, identity, source));
                        if let Some(lease) = live_handle_lease.take() {
                            let prior = self.filesystem_live_handle_leases.insert(identity, lease);
                            debug_assert!(prior.is_none());
                        }
                    }
                }

                if let Some(retirement) = plan.retirement
                    && retirement.success.accepts(result)
                {
                    let input = self.filesystem_operation_attempts[attempt_index]
                        .logical_handle_inputs
                        .iter()
                        .find(|input| input.operand_ordinal() == retirement.operand_ordinal)
                        .copied()
                        .expect("retirement plan must name one retained handle input");
                    let FilesystemLogicalHandleInputResolution::Resolved(identity) =
                        input.resolution()
                    else {
                        return trap(
                            "filesystem provider closed an unresolved logical handle token",
                        );
                    };
                    let retired = self
                        .filesystem_logical_handles
                        .retire(input.kind(), identity);
                    for retired_identity in &retired {
                        self.filesystem_live_handle_leases.remove(retired_identity);
                    }
                    self.filesystem_operation_attempts[attempt_index].retired_logical_handles =
                        retired;
                }
                Ok(())
            }

            pub(crate) fn reserve_prepared_live_filesystem_handle(
                &self,
                plan: &PreparedFilesystemLogicalHandlePlan,
            ) -> EvalResult<Option<BuildEvaluationLiveFilesystemHandleLease>> {
                let owns_new_resource = matches!(
                    plan.output,
                    Some(PreparedFilesystemLogicalHandleOutput::Created { .. })
                        | Some(PreparedFilesystemLogicalHandleOutput::Duplicated { .. })
                );
                if !owns_new_resource {
                    return Ok(None);
                }
                self.build_evaluation_sponsor
                    .as_ref()
                    .map(BuildEvaluationSponsor::reserve_live_filesystem_handle)
                    .transpose()
                    .map_err(Halt::Resource)
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
                    .find(|input| {
                        input.operand_ordinal() == operand_ordinal && input.kind() == kind
                    })
                    .and_then(|input| match input.resolution() {
                        FilesystemLogicalHandleInputResolution::Resolved(identity) => {
                            Some(identity)
                        }
                        FilesystemLogicalHandleInputResolution::Null
                        | FilesystemLogicalHandleInputResolution::Unknown => None,
                    })
            }
        }

        fn filesystem_logical_handle_halt(
            error: logical_handle_store::FilesystemLogicalHandleError,
        ) -> Halt {
            use crate::interpreter::evaluator::filesystem::logical_handle_store::FilesystemLogicalHandleError as Error;
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
    }

    pub(crate) mod observations {
        use crate::interpreter::evaluator::{EvalResult, Evaluator, STAT_OUTPUT_BYTES, trap};
        use crate::{FilesystemMetadataField, FilesystemMetadataObservation};

        impl<'program> Evaluator<'program> {
            pub(crate) fn canonical_metadata_carrier(
                &self,
                observation: FilesystemMetadataObservation,
                capacity: usize,
                context: &str,
            ) -> EvalResult<Vec<u8>> {
                if capacity < STAT_OUTPUT_BYTES
                    || self.filesystem_metadata_layout.record_size() > capacity
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
        }
    }

    pub(crate) mod preparation {
        pub(crate) use logical_handle_plans::{
            PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
        };
        pub(crate) use prepared_calls::PreparedFilesystemCall;
        pub(crate) use prepared_outputs::{PreparedByteOutput, PreparedTransferCount};

        use crate::interpreter::evaluator::{EvalResult, FilesystemHostOperation, Halt, trap};

        pub(crate) const MAX_FILESYSTEM_TRANSFER_BYTES: usize = 16 * 1024 * 1024;

        const FILETIME_BYTES: usize = 8;

        pub(crate) const FIND_DATA_OUTPUT_BYTES: usize = 320;

        const OVERLAPPED_BYTES: usize = 32;

        const PATH_MAX_OUTPUT_BYTES: usize = 1024;

        pub(crate) const STAT_OUTPUT_BYTES: usize = crate::FILESYSTEM_METADATA_API_CARRIER_BYTES;

        const TIMESPEC_PAIR_BYTES: usize = 32;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum FilesystemTransferCountError {
            NegativeOrUnrepresentable,
            ExceedsEvaluatorLimit,
        }

        pub(crate) fn checked_filesystem_transfer_count(
            raw: i64,
        ) -> Result<PreparedTransferCount, FilesystemTransferCountError> {
            let host = usize::try_from(raw)
                .map_err(|_| FilesystemTransferCountError::NegativeOrUnrepresentable)?;
            if host > MAX_FILESYSTEM_TRANSFER_BYTES {
                return Err(FilesystemTransferCountError::ExceedsEvaluatorLimit);
            }
            Ok(PreparedTransferCount { host })
        }

        fn check_byte_len(length: usize) -> EvalResult<()> {
            if length > MAX_FILESYSTEM_TRANSFER_BYTES {
                return Err(Halt::Trap(format!(
                    "filesystem byte argument exceeds evaluator limit of {MAX_FILESYSTEM_TRANSFER_BYTES} bytes"
                )));
            }
            Ok(())
        }

        fn checked_relative_component(bytes: Vec<u8>) -> EvalResult<Vec<u8>> {
            if bytes.is_empty()
                || bytes == b"."
                || bytes == b".."
                || bytes.contains(&b'/')
                || bytes.contains(&b'\\')
                || bytes.contains(&0)
            {
                return trap(
                    "filesystem relative-name operand is not one nonempty portable path component",
                );
            }
            Ok(bytes)
        }

        pub(crate) fn synthetic_handle_fd(handle: i64) -> Option<i32> {
            i32::try_from(handle).ok()
        }

        fn check_filesystem_arity(
            operation: FilesystemHostOperation,
            actual: usize,
        ) -> EvalResult<()> {
            let expected = operation.operand_kinds().len();
            if actual != expected {
                return trap(format!(
                    "canonical filesystem operation `{operation}` expects {expected} operand(s), got {actual}"
                ));
            }
            Ok(())
        }

        fn rooted_package_build_operation_refusal(
            operation: FilesystemHostOperation,
        ) -> Option<&'static str> {
            match operation {
                FilesystemHostOperation::Canonicalize => Some("would expose a host-absolute path"),
                FilesystemHostOperation::FindFirst
                | FilesystemHostOperation::FindNext
                | FilesystemHostOperation::FindClose => Some(
                    "belongs to an unrooted find-cursor protocol not admitted by the Build facet",
                ),
                _ => None,
            }
        }

        pub(crate) mod logical_handle_plans {
            use crate::interpreter::evaluator::FilesystemLogicalHandleKind;

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct PreparedFilesystemLogicalHandleInput {
                pub(crate) operand_ordinal: u8,
                pub(crate) kind: FilesystemLogicalHandleKind,
                pub(crate) raw: i64,
                pub(crate) null_allowed: bool,
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum FilesystemLogicalHandleResultSuccess {
                NonNegative,
                NotMinusOne,
                Zero,
                NonZero,
            }

            impl FilesystemLogicalHandleResultSuccess {
                pub(crate) const fn accepts(self, result: i64) -> bool {
                    match self {
                        Self::NonNegative => result >= 0,
                        Self::NotMinusOne => result != -1,
                        Self::Zero => result == 0,
                        Self::NonZero => result != 0,
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum PreparedFilesystemLogicalHandleOutput {
                Created {
                    kind: FilesystemLogicalHandleKind,
                    success: FilesystemLogicalHandleResultSuccess,
                },
                Duplicated {
                    source_operand_ordinal: u8,
                    success: FilesystemLogicalHandleResultSuccess,
                },
                Borrowed {
                    source_operand_ordinal: u8,
                    success: FilesystemLogicalHandleResultSuccess,
                },
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum FilesystemLogicalHandleRetirementSuccess {
                Zero,
                NonZero,
            }

            impl FilesystemLogicalHandleRetirementSuccess {
                pub(crate) const fn accepts(self, result: i64) -> bool {
                    match self {
                        Self::Zero => result == 0,
                        Self::NonZero => result != 0,
                    }
                }
            }

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct PreparedFilesystemLogicalHandleRetirement {
                pub(crate) operand_ordinal: u8,
                pub(crate) success: FilesystemLogicalHandleRetirementSuccess,
            }

            pub(crate) struct PreparedFilesystemLogicalHandlePlan {
                pub(crate) inputs: Vec<PreparedFilesystemLogicalHandleInput>,
                pub(crate) input_success: Option<FilesystemLogicalHandleResultSuccess>,
                pub(crate) output: Option<PreparedFilesystemLogicalHandleOutput>,
                pub(crate) retirement: Option<PreparedFilesystemLogicalHandleRetirement>,
            }
        }

        pub(crate) mod prepared_calls {
            use crate::interpreter::evaluator::filesystem::preparation::MAX_FILESYSTEM_TRANSFER_BYTES;
            use crate::interpreter::evaluator::filesystem::preparation::logical_handle_plans::{
                FilesystemLogicalHandleResultSuccess, FilesystemLogicalHandleRetirementSuccess,
                PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandleOutput,
                PreparedFilesystemLogicalHandlePlan, PreparedFilesystemLogicalHandleRetirement,
            };
            use crate::interpreter::evaluator::filesystem::preparation::prepared_outputs::{
                PreparedByteOutput, PreparedI64Output, PreparedMutableByteInput,
                PreparedTransferCount, prepared_byte,
            };
            use crate::interpreter::evaluator::filesystem::preparation::{
                FILETIME_BYTES, FIND_DATA_OUTPUT_BYTES, FilesystemTransferCountError,
                OVERLAPPED_BYTES, PATH_MAX_OUTPUT_BYTES, STAT_OUTPUT_BYTES, TIMESPEC_PAIR_BYTES,
                check_byte_len, check_filesystem_arity, checked_filesystem_transfer_count,
                checked_relative_component, rooted_package_build_operation_refusal,
            };
            use crate::interpreter::evaluator::{
                EvalResult, Evaluator, ExpressionHandle, FilesystemHostOperation,
                FilesystemLogicalHandleKind, Frame, Halt, Value, rooted_build_path_parts, trap,
                unsupported, validate_build_relative_path,
            };

            pub(crate) enum PreparedFilesystemPath {
                Native(Vec<u8>),
            }

            impl PreparedFilesystemPath {
                pub(crate) fn into_native(self) -> EvalResult<Vec<u8>> {
                    match self {
                        Self::Native(bytes) => Ok(bytes),
                    }
                }
            }

            impl std::ops::Deref for PreparedFilesystemPath {
                type Target = [u8];
                fn deref(&self) -> &[u8] {
                    match self {
                        Self::Native(bytes) => bytes,
                    }
                }
            }

            pub(crate) enum PreparedFilesystemCall {
                Create {
                    path: PreparedFilesystemPath,
                },
                Open {
                    path: PreparedFilesystemPath,
                    flags: i32,
                },
                OpenCreate {
                    path: PreparedFilesystemPath,
                    flags: i32,
                    mode: i32,
                },
                Read {
                    fd: i32,
                    buffer: PreparedByteOutput,
                    count: PreparedTransferCount,
                },
                Write {
                    fd: i32,
                    bytes: Vec<u8>,
                },
                ReadAt {
                    fd: i32,
                    buffer: PreparedByteOutput,
                    count: PreparedTransferCount,
                    offset: i64,
                },
                WriteAt {
                    fd: i32,
                    bytes: Vec<u8>,
                    offset: i64,
                },
                Close {
                    fd: i32,
                },
                Remove {
                    path: PreparedFilesystemPath,
                },
                Seek {
                    fd: i32,
                    offset: i64,
                    whence: i32,
                },
                CreateDir {
                    path: PreparedFilesystemPath,
                },
                RemoveDir {
                    path: PreparedFilesystemPath,
                },
                CreateDirName {
                    name: PreparedFilesystemPath,
                },
                OpenAt {
                    dirfd: i32,
                    name: Vec<u8>,
                    flags: i32,
                },
                UnlinkAt {
                    dirfd: i32,
                    name: Vec<u8>,
                    flags: i32,
                },
                SetPermissions {
                    path: PreparedFilesystemPath,
                    mode: u32,
                },
                SetFilePermissions {
                    fd: i32,
                    mode: u32,
                },
                Rename {
                    from: PreparedFilesystemPath,
                    to: PreparedFilesystemPath,
                },
                HardLink {
                    original: PreparedFilesystemPath,
                    link: PreparedFilesystemPath,
                },
                Symlink {
                    target: Vec<u8>,
                    link: PreparedFilesystemPath,
                },
                ReadLink {
                    path: PreparedFilesystemPath,
                    buffer: PreparedByteOutput,
                    count: PreparedTransferCount,
                },
                Canonicalize {
                    path: PreparedFilesystemPath,
                    buffer: PreparedByteOutput,
                },
                ReadDir {
                    fd: i32,
                    buffer: PreparedByteOutput,
                    count: PreparedTransferCount,
                    position: PreparedI64Output,
                },
                FindFirst {
                    pattern: Vec<u8>,
                    data: PreparedByteOutput,
                },
                FindNext {
                    handle: i64,
                    data: PreparedByteOutput,
                },
                FindClose {
                    handle: i64,
                },
                CreateHardLink {
                    link: PreparedFilesystemPath,
                    existing: PreparedFilesystemPath,
                },
                OpenPathHandle {
                    path: PreparedFilesystemPath,
                    template_file: i64,
                },
                CloseHandle {
                    handle: i64,
                },
                GetOsfHandle {
                    fd: i32,
                },
                FinalPathNameByHandle {
                    handle: i64,
                    buffer: PreparedByteOutput,
                    capacity: PreparedTransferCount,
                },
                SetFileTime {
                    handle: i64,
                    last_write: Vec<u8>,
                },
                LockFileEx {
                    handle: i64,
                    flags: u32,
                },
                UnlockFile {
                    handle: i64,
                },
                GetLastError,
                RemoveName {
                    path: PreparedFilesystemPath,
                },
                RemoveDirName {
                    path: PreparedFilesystemPath,
                },
                ReadMetadata {
                    path: PreparedFilesystemPath,
                    buffer: PreparedByteOutput,
                },
                ReadFileMetadata {
                    fd: i32,
                    buffer: PreparedByteOutput,
                },
                ReadSymlinkMetadata {
                    path: PreparedFilesystemPath,
                    buffer: PreparedByteOutput,
                },
                SetLen {
                    fd: i32,
                    length: i64,
                },
                SetFileTimes {
                    fd: i32,
                    times: PreparedMutableByteInput,
                },
                Sync {
                    fd: i32,
                },
                SyncData {
                    fd: i32,
                },
                Duplicate {
                    fd: i32,
                },
                LockFile {
                    fd: i32,
                    operation: i32,
                },
                ChangeOwner {
                    path: PreparedFilesystemPath,
                    uid: i32,
                    gid: i32,
                },
                ChangeOwnerNoFollow {
                    path: PreparedFilesystemPath,
                    uid: i32,
                    gid: i32,
                },
                ChangeFileOwner {
                    fd: i32,
                    uid: i32,
                    gid: i32,
                },
                Errno,
            }

            impl PreparedFilesystemCall {
                pub(crate) fn validate_output_carriers(&self) -> EvalResult<()> {
                    match self {
                        Self::Read { buffer, .. }
                        | Self::ReadAt { buffer, .. }
                        | Self::ReadLink { buffer, .. }
                        | Self::Canonicalize { buffer, .. }
                        | Self::FinalPathNameByHandle { buffer, .. }
                        | Self::ReadMetadata { buffer, .. }
                        | Self::ReadFileMetadata { buffer, .. }
                        | Self::ReadSymlinkMetadata { buffer, .. } => buffer.validate_contents(),
                        Self::ReadDir {
                            buffer, position, ..
                        } => {
                            buffer.validate_contents()?;
                            position.validate_contents()
                        }
                        Self::FindFirst { data, .. } | Self::FindNext { data, .. } => {
                            data.validate_contents()
                        }
                        _ => Ok(()),
                    }
                }

                pub(crate) fn logical_handle_plan(&self) -> PreparedFilesystemLogicalHandlePlan {
                    use FilesystemLogicalHandleKind as Kind;
                    use FilesystemLogicalHandleResultSuccess as ResultSuccess;
                    use FilesystemLogicalHandleRetirementSuccess as RetireSuccess;

                    let input = |operand_ordinal, kind, raw, null_allowed| {
                        PreparedFilesystemLogicalHandleInput {
                            operand_ordinal,
                            kind,
                            raw,
                            null_allowed,
                        }
                    };
                    let mut inputs = Vec::new();
                    let mut input_success = None;
                    let mut output = None;
                    let mut retirement = None;
                    match self {
                        Self::Create { .. } | Self::Open { .. } | Self::OpenCreate { .. } => {
                            output = Some(PreparedFilesystemLogicalHandleOutput::Created {
                                kind: Kind::Descriptor,
                                success: ResultSuccess::NonNegative,
                            });
                        }
                        Self::Read { fd, .. }
                        | Self::Write { fd, .. }
                        | Self::ReadAt { fd, .. }
                        | Self::WriteAt { fd, .. }
                        | Self::Seek { fd, .. }
                        | Self::SetFilePermissions { fd, .. }
                        | Self::ReadDir { fd, .. }
                        | Self::GetOsfHandle { fd }
                        | Self::ReadFileMetadata { fd, .. }
                        | Self::SetLen { fd, .. }
                        | Self::SetFileTimes { fd, .. }
                        | Self::Sync { fd }
                        | Self::SyncData { fd }
                        | Self::Duplicate { fd }
                        | Self::LockFile { fd, .. }
                        | Self::ChangeFileOwner { fd, .. } => {
                            inputs.push(input(0, Kind::Descriptor, i64::from(*fd), false));
                            input_success = Some(match self {
                                Self::Read { .. }
                                | Self::Write { .. }
                                | Self::ReadAt { .. }
                                | Self::WriteAt { .. }
                                | Self::Seek { .. }
                                | Self::ReadDir { .. }
                                | Self::GetOsfHandle { .. }
                                | Self::Duplicate { .. } => ResultSuccess::NonNegative,
                                Self::SetFilePermissions { .. }
                                | Self::ReadFileMetadata { .. }
                                | Self::SetLen { .. }
                                | Self::SetFileTimes { .. }
                                | Self::Sync { .. }
                                | Self::SyncData { .. }
                                | Self::LockFile { .. }
                                | Self::ChangeFileOwner { .. } => ResultSuccess::Zero,
                                _ => unreachable!("descriptor-input operation group is exhaustive"),
                            });
                            output = match self {
                                Self::GetOsfHandle { .. } => {
                                    Some(PreparedFilesystemLogicalHandleOutput::Borrowed {
                                        source_operand_ordinal: 0,
                                        success: ResultSuccess::NonNegative,
                                    })
                                }
                                Self::Duplicate { .. } => {
                                    Some(PreparedFilesystemLogicalHandleOutput::Duplicated {
                                        source_operand_ordinal: 0,
                                        success: ResultSuccess::NonNegative,
                                    })
                                }
                                _ => None,
                            };
                        }
                        Self::Close { fd } => {
                            inputs.push(input(0, Kind::Descriptor, i64::from(*fd), false));
                            input_success = Some(ResultSuccess::Zero);
                            retirement = Some(PreparedFilesystemLogicalHandleRetirement {
                                operand_ordinal: 0,
                                success: RetireSuccess::Zero,
                            });
                        }
                        Self::OpenAt { dirfd, .. } => {
                            inputs.push(input(0, Kind::Descriptor, i64::from(*dirfd), false));
                            input_success = Some(ResultSuccess::NonNegative);
                            output = Some(PreparedFilesystemLogicalHandleOutput::Created {
                                kind: Kind::Descriptor,
                                success: ResultSuccess::NonNegative,
                            });
                        }
                        Self::UnlinkAt { dirfd, .. } => {
                            inputs.push(input(0, Kind::Descriptor, i64::from(*dirfd), false));
                            input_success = Some(ResultSuccess::Zero);
                        }
                        Self::FindFirst { .. } => {
                            output = Some(PreparedFilesystemLogicalHandleOutput::Created {
                                kind: Kind::Find,
                                success: ResultSuccess::NotMinusOne,
                            });
                        }
                        Self::FindNext { handle, .. } => {
                            inputs.push(input(0, Kind::Find, *handle, false));
                            input_success = Some(ResultSuccess::NonZero);
                        }
                        Self::FindClose { handle } => {
                            inputs.push(input(0, Kind::Find, *handle, false));
                            input_success = Some(ResultSuccess::NonZero);
                            retirement = Some(PreparedFilesystemLogicalHandleRetirement {
                                operand_ordinal: 0,
                                success: RetireSuccess::NonZero,
                            });
                        }
                        Self::OpenPathHandle { template_file, .. } => {
                            inputs.push(input(6, Kind::Native, *template_file, true));
                            input_success = Some(ResultSuccess::NotMinusOne);
                            output = Some(PreparedFilesystemLogicalHandleOutput::Created {
                                kind: Kind::Native,
                                success: ResultSuccess::NotMinusOne,
                            });
                        }
                        Self::CloseHandle { handle } => {
                            inputs.push(input(0, Kind::Native, *handle, false));
                            input_success = Some(ResultSuccess::NonZero);
                            retirement = Some(PreparedFilesystemLogicalHandleRetirement {
                                operand_ordinal: 0,
                                success: RetireSuccess::NonZero,
                            });
                        }
                        Self::FinalPathNameByHandle { handle, .. }
                        | Self::SetFileTime { handle, .. }
                        | Self::LockFileEx { handle, .. }
                        | Self::UnlockFile { handle, .. } => {
                            inputs.push(input(0, Kind::Native, *handle, false));
                            input_success = Some(ResultSuccess::NonZero);
                        }
                        Self::Remove { .. }
                        | Self::CreateDir { .. }
                        | Self::RemoveDir { .. }
                        | Self::CreateDirName { .. }
                        | Self::SetPermissions { .. }
                        | Self::Rename { .. }
                        | Self::HardLink { .. }
                        | Self::Symlink { .. }
                        | Self::ReadLink { .. }
                        | Self::Canonicalize { .. }
                        | Self::CreateHardLink { .. }
                        | Self::GetLastError
                        | Self::RemoveName { .. }
                        | Self::RemoveDirName { .. }
                        | Self::ReadMetadata { .. }
                        | Self::ReadSymlinkMetadata { .. }
                        | Self::ChangeOwner { .. }
                        | Self::ChangeOwnerNoFollow { .. }
                        | Self::Errno => {}
                    }
                    PreparedFilesystemLogicalHandlePlan {
                        inputs,
                        input_success,
                        output,
                        retirement,
                    }
                }
            }

            struct FilesystemArgumentCursor<'evaluation, 'program, 'arguments, 'frame> {
                evaluator: &'evaluation mut Evaluator<'program>,
                attempt_index: usize,
                arguments: std::slice::Iter<'arguments, ExpressionHandle>,
                frame: &'frame mut Frame,
                consumed: usize,
            }

            impl<'evaluation, 'program, 'arguments, 'frame>
                FilesystemArgumentCursor<'evaluation, 'program, 'arguments, 'frame>
            {
                fn new(
                    evaluator: &'evaluation mut Evaluator<'program>,
                    attempt_index: usize,
                    arguments: &'arguments [ExpressionHandle],
                    frame: &'frame mut Frame,
                ) -> Self {
                    Self {
                        evaluator,
                        attempt_index,
                        arguments: arguments.iter(),
                        frame,
                        consumed: 0,
                    }
                }

                fn next(&mut self) -> EvalResult<(u8, ExpressionHandle)> {
                    let operand_ordinal = u8::try_from(self.consumed).map_err(|_| {
                        Halt::Trap("canonical filesystem operand ordinal exceeds u8".to_owned())
                    })?;
                    let handle = self.arguments.next().copied().ok_or_else(|| {
                        Halt::Trap(
                            "canonical filesystem call is missing an authored operand".to_owned(),
                        )
                    })?;
                    self.consumed += 1;
                    Ok((operand_ordinal, handle))
                }

                fn value(&mut self) -> EvalResult<(u8, Value)> {
                    let (operand_ordinal, handle) = self.next()?;
                    self.evaluator
                        .eval_expression(handle, self.frame)
                        .map(|value| (operand_ordinal, value))
                }

                fn integer(&mut self) -> EvalResult<(u8, i64)> {
                    match self.value()? {
                        (operand_ordinal, Value::Int(value)) => Ok((operand_ordinal, value)),
                        _ => trap("canonical filesystem scalar operand is not an integer"),
                    }
                }

                fn i64(&mut self) -> EvalResult<i64> {
                    let (_, value) = self.integer()?;
                    Ok(value)
                }

                fn logical_handle(
                    &mut self,
                    kind: FilesystemLogicalHandleKind,
                    null_allowed: bool,
                ) -> EvalResult<i64> {
                    let (operand_ordinal, raw) = self.integer()?;
                    self.evaluator
                        .record_prepared_filesystem_logical_handle_input(
                            self.attempt_index,
                            operand_ordinal,
                            kind,
                            raw,
                            null_allowed,
                        );
                    Ok(raw)
                }

                fn find_handle(&mut self) -> EvalResult<i64> {
                    self.logical_handle(FilesystemLogicalHandleKind::Find, false)
                }

                fn native_handle(&mut self) -> EvalResult<i64> {
                    self.logical_handle(FilesystemLogicalHandleKind::Native, false)
                }

                fn nullable_native_handle(&mut self) -> EvalResult<i64> {
                    self.logical_handle(FilesystemLogicalHandleKind::Native, true)
                }

                fn i32(&mut self) -> EvalResult<i32> {
                    let (_, raw) = self.integer()?;
                    i32::try_from(raw).map_err(|_| {
                        Halt::Trap(format!(
                            "canonical filesystem i32 operand `{raw}` is out of range"
                        ))
                    })
                }

                fn u32(&mut self) -> EvalResult<u32> {
                    let (_, raw) = self.integer()?;
                    u32::try_from(raw).map_err(|_| {
                        Halt::Trap(format!(
                            "canonical filesystem u32 operand `{raw}` is out of range"
                        ))
                    })
                }

                fn descriptor(&mut self) -> EvalResult<i32> {
                    let (operand_ordinal, value) = self.value()?;
                    let raw = match &value {
                        Value::Struct { fields, .. } => fields.get("fd").and_then(|cell| {
                            let value = cell.borrow();
                            match *value {
                                Value::Int(fd) => Some(fd),
                                _ => None,
                            }
                        }),
                        Value::Int(fd) => Some(*fd),
                        _ => None,
                    }
                    .ok_or_else(|| {
                        Halt::Trap("filesystem call file handle is not an fd".to_owned())
                    })?;
                    let descriptor = i32::try_from(raw).map_err(|_| {
                        Halt::Trap(format!("filesystem fd `{raw}` is out of range"))
                    })?;
                    self.evaluator
                        .record_prepared_filesystem_logical_handle_input(
                            self.attempt_index,
                            operand_ordinal,
                            FilesystemLogicalHandleKind::Descriptor,
                            i64::from(descriptor),
                            false,
                        );
                    Ok(descriptor)
                }

                fn count(&mut self) -> EvalResult<PreparedTransferCount> {
                    let (_, raw) = self.integer()?;
                    checked_filesystem_transfer_count(raw).map_err(|error| match error {
            FilesystemTransferCountError::NegativeOrUnrepresentable => Halt::Trap(
                "filesystem transfer count is negative or not host-representable".to_owned(),
            ),
            FilesystemTransferCountError::ExceedsEvaluatorLimit => Halt::Trap(format!(
                "filesystem transfer count exceeds evaluator limit of {MAX_FILESYSTEM_TRANSFER_BYTES} bytes"
            )),
        })
                }

                fn raw_bytes(&mut self) -> EvalResult<(u8, Vec<u8>)> {
                    let (operand_ordinal, value) = self.value()?;
                    if let Some((_, relative)) = rooted_build_path_parts(&value)? {
                        check_byte_len(relative.len())?;
                        return Ok((operand_ordinal, relative));
                    }
                    let bytes = match value {
                        Value::Str(text) => {
                            let text = text.borrow();
                            check_byte_len(text.len())?;
                            Ok(text.to_vec())
                        }
                        Value::Array(cells) => {
                            check_byte_len(cells.len())?;
                            cells.iter().map(prepared_byte).collect()
                        }
                        Value::Ref(reference) if reference.storage.is_none() => {
                            match &*reference.cell.borrow() {
                                Value::Array(cells) => {
                                    check_byte_len(cells.len())?;
                                    cells.iter().map(prepared_byte).collect()
                                }
                                other => unsupported(format!(
                                    "filesystem call expected byte data behind a reference, got {other:?}"
                                )),
                            }
                        }
                        other => unsupported(format!(
                            "filesystem call expected byte data, got {other:?}"
                        )),
                    }?;
                    Ok((operand_ordinal, bytes))
                }

                fn bytes(&mut self) -> EvalResult<Vec<u8>> {
                    let (_, bytes) = self.raw_bytes()?;
                    Ok(bytes)
                }

                fn path_like_bytes(&mut self) -> EvalResult<Vec<u8>> {
                    let (_, bytes) = self.raw_bytes()?;
                    Ok(bytes)
                }

                fn relative_component(&mut self) -> EvalResult<Vec<u8>> {
                    let (_, bytes) = self.raw_bytes()?;
                    let bytes = checked_relative_component(bytes)?;
                    Ok(bytes)
                }

                fn path(&mut self) -> EvalResult<PreparedFilesystemPath> {
                    let (operand_ordinal, value) = self.value()?;
                    let Some((root, relative)) = rooted_build_path_parts(&value)? else {
                        return match value {
                            Value::Str(text) => {
                                let text = text.borrow();
                                check_byte_len(text.len())?;
                                Ok(PreparedFilesystemPath::Native(text.to_vec()))
                            }
                            Value::Array(cells) => {
                                check_byte_len(cells.len())?;
                                cells
                                    .iter()
                                    .map(prepared_byte)
                                    .collect::<EvalResult<Vec<_>>>()
                                    .map(PreparedFilesystemPath::Native)
                            }
                            other => unsupported(format!(
                                "filesystem call expected path byte data, got {other:?}"
                            )),
                        };
                    };
                    validate_build_relative_path(&relative)?;
                    // Build-time evaluation supplies only the virtual filesystem; a
                    // rooted path needs a scoped host grant no caller can provide here.
                    let _ = (operand_ordinal, root);
                    trap("rooted build path requires a scoped real filesystem")
                }

                fn mutable_bytes(&mut self) -> EvalResult<PreparedByteOutput> {
                    let (_, handle) = self.next()?;
                    let cell = self.evaluator.resolve_place(handle, self.frame)?;
                    let cell = self.evaluator.deref_cell(cell);
                    let output = {
                        let value = cell.borrow();
                        match &*value {
                            Value::Str(text) => {
                                let capacity = text.borrow().len();
                                check_byte_len(capacity)?;
                                Ok(PreparedByteOutput::Text {
                                    text: text.clone(),
                                    capacity,
                                })
                            }
                            Value::Array(cells) => {
                                check_byte_len(cells.len())?;
                                Ok(PreparedByteOutput::Array(cells.clone()))
                            }
                            other => trap(format!(
                                "filesystem mutable byte operand has invalid shape {other:?}"
                            )),
                        }
                    }?;
                    output.validate_contents()?;
                    Ok(output)
                }

                fn mutable_i64(&mut self) -> EvalResult<PreparedI64Output> {
                    let (_, handle) = self.next()?;
                    let cell = self.evaluator.resolve_place(handle, self.frame)?;
                    let cell = self.evaluator.deref_cell(cell);
                    let initial = match *cell.borrow() {
                        Value::Int(value) => value,
                        _ => return trap("filesystem mutable scalar operand is not an integer"),
                    };
                    Ok(PreparedI64Output { cell, initial })
                }

                fn mutable_byte_input(
                    &mut self,
                    required_bytes: usize,
                ) -> EvalResult<PreparedMutableByteInput> {
                    let output = self.mutable_bytes()?;
                    output.require_capacity(required_bytes)?;
                    let bytes = output.snapshot()?;
                    Ok(PreparedMutableByteInput { bytes })
                }

                fn finish(self) -> EvalResult<()> {
                    if self.arguments.len() == 0 {
                        Ok(())
                    } else {
                        trap("canonical filesystem call has unconsumed authored operands")
                    }
                }
            }

            impl<'program> Evaluator<'program> {
                pub(crate) fn prepare_filesystem_call(
                    &mut self,
                    operation: FilesystemHostOperation,
                    arguments: &[ExpressionHandle],
                    frame: &mut Frame,
                ) -> EvalResult<PreparedFilesystemCall> {
                    check_filesystem_arity(operation, arguments.len())?;
                    if self.rooted_build_paths_required
                        && let Some(reason) = rooted_package_build_operation_refusal(operation)
                    {
                        return Err(Halt::Trap(format!(
                            "package build filesystem operation `{}` {reason}",
                            operation.canonical_name(),
                        )));
                    }
                    let attempt_index = *self
                        .filesystem_operation_attempt_stack
                        .last()
                        .expect("filesystem preparation requires an active operation attempt");
                    let mut a =
                        FilesystemArgumentCursor::new(self, attempt_index, arguments, frame);
                    let call = match operation {
                        FilesystemHostOperation::Create => {
                            let path = a.path()?;
                            a.i32()?;
                            PreparedFilesystemCall::Create { path }
                        }
                        FilesystemHostOperation::Open => PreparedFilesystemCall::Open {
                            path: a.path()?,
                            flags: a.i32()?,
                        },
                        FilesystemHostOperation::OpenCreate => PreparedFilesystemCall::OpenCreate {
                            path: a.path()?,
                            flags: a.i32()?,
                            mode: a.i32()?,
                        },
                        FilesystemHostOperation::Read => {
                            let fd = a.descriptor()?;
                            let buffer = a.mutable_bytes()?;
                            let count = a.count()?;
                            buffer.require_capacity(count.host)?;
                            PreparedFilesystemCall::Read { fd, buffer, count }
                        }
                        FilesystemHostOperation::Write => PreparedFilesystemCall::Write {
                            fd: a.descriptor()?,
                            bytes: a.bytes()?,
                        },
                        FilesystemHostOperation::ReadAt => {
                            let fd = a.descriptor()?;
                            let buffer = a.mutable_bytes()?;
                            let count = a.count()?;
                            buffer.require_capacity(count.host)?;
                            let offset = a.i64()?;
                            PreparedFilesystemCall::ReadAt {
                                fd,
                                buffer,
                                count,
                                offset,
                            }
                        }
                        FilesystemHostOperation::WriteAt => PreparedFilesystemCall::WriteAt {
                            fd: a.descriptor()?,
                            bytes: a.bytes()?,
                            offset: a.i64()?,
                        },
                        FilesystemHostOperation::Close => PreparedFilesystemCall::Close {
                            fd: a.descriptor()?,
                        },
                        FilesystemHostOperation::Remove => {
                            PreparedFilesystemCall::Remove { path: a.path()? }
                        }
                        FilesystemHostOperation::Seek => PreparedFilesystemCall::Seek {
                            fd: a.descriptor()?,
                            offset: a.i64()?,
                            whence: a.i32()?,
                        },
                        FilesystemHostOperation::CreateDir => {
                            let path = a.path()?;
                            a.i32()?;
                            PreparedFilesystemCall::CreateDir { path }
                        }
                        FilesystemHostOperation::RemoveDir => {
                            PreparedFilesystemCall::RemoveDir { path: a.path()? }
                        }
                        FilesystemHostOperation::CreateDirName => {
                            let name = PreparedFilesystemPath::Native(a.path_like_bytes()?);
                            a.i32()?;
                            PreparedFilesystemCall::CreateDirName { name }
                        }
                        FilesystemHostOperation::OpenAt => {
                            let dirfd = a.descriptor()?;
                            let name = a.relative_component()?;
                            let flags = a.i32()?;
                            PreparedFilesystemCall::OpenAt { dirfd, name, flags }
                        }
                        FilesystemHostOperation::UnlinkAt => {
                            let dirfd = a.descriptor()?;
                            let name = a.relative_component()?;
                            let flags = a.i32()?;
                            PreparedFilesystemCall::UnlinkAt { dirfd, name, flags }
                        }
                        FilesystemHostOperation::SetPermissions => {
                            PreparedFilesystemCall::SetPermissions {
                                path: a.path()?,
                                mode: a.u32()?,
                            }
                        }
                        FilesystemHostOperation::SetFilePermissions => {
                            PreparedFilesystemCall::SetFilePermissions {
                                fd: a.descriptor()?,
                                mode: a.u32()?,
                            }
                        }
                        FilesystemHostOperation::Rename => PreparedFilesystemCall::Rename {
                            from: a.path()?,
                            to: a.path()?,
                        },
                        FilesystemHostOperation::HardLink => PreparedFilesystemCall::HardLink {
                            original: a.path()?,
                            link: a.path()?,
                        },
                        FilesystemHostOperation::Symlink => PreparedFilesystemCall::Symlink {
                            target: a.path_like_bytes()?,
                            link: a.path()?,
                        },
                        FilesystemHostOperation::ReadLink => {
                            let path = a.path()?;
                            let buffer = a.mutable_bytes()?;
                            let count = a.count()?;
                            buffer.require_capacity(count.host)?;
                            PreparedFilesystemCall::ReadLink {
                                path,
                                buffer,
                                count,
                            }
                        }
                        FilesystemHostOperation::Canonicalize => {
                            let path = a.path()?;
                            let buffer = a.mutable_bytes()?;
                            buffer.require_capacity(PATH_MAX_OUTPUT_BYTES)?;
                            PreparedFilesystemCall::Canonicalize { path, buffer }
                        }
                        FilesystemHostOperation::ReadDir => {
                            let fd = a.descriptor()?;
                            let buffer = a.mutable_bytes()?;
                            let count = a.count()?;
                            buffer.require_capacity(count.host)?;
                            let position = a.mutable_i64()?;
                            PreparedFilesystemCall::ReadDir {
                                fd,
                                buffer,
                                count,
                                position,
                            }
                        }
                        FilesystemHostOperation::FindFirst => {
                            let pattern = a.path_like_bytes()?;
                            let data = a.mutable_bytes()?;
                            data.require_capacity(FIND_DATA_OUTPUT_BYTES)?;
                            PreparedFilesystemCall::FindFirst { pattern, data }
                        }
                        FilesystemHostOperation::FindNext => {
                            let handle = a.find_handle()?;
                            let data = a.mutable_bytes()?;
                            data.require_capacity(FIND_DATA_OUTPUT_BYTES)?;
                            PreparedFilesystemCall::FindNext { handle, data }
                        }
                        FilesystemHostOperation::FindClose => PreparedFilesystemCall::FindClose {
                            handle: a.find_handle()?,
                        },
                        FilesystemHostOperation::CreateHardLink => {
                            let link = a.path()?;
                            let existing = a.path()?;
                            a.i64()?;
                            PreparedFilesystemCall::CreateHardLink { link, existing }
                        }
                        FilesystemHostOperation::OpenPathHandle => {
                            let path = a.path()?;
                            a.u32()?;
                            a.u32()?;
                            a.i64()?;
                            a.u32()?;
                            a.u32()?;
                            PreparedFilesystemCall::OpenPathHandle {
                                path,
                                template_file: a.nullable_native_handle()?,
                            }
                        }
                        FilesystemHostOperation::CloseHandle => {
                            PreparedFilesystemCall::CloseHandle {
                                handle: a.native_handle()?,
                            }
                        }
                        FilesystemHostOperation::GetOsfHandle => {
                            PreparedFilesystemCall::GetOsfHandle {
                                fd: a.descriptor()?,
                            }
                        }
                        FilesystemHostOperation::FinalPathNameByHandle => {
                            let handle = a.native_handle()?;
                            let buffer = a.mutable_bytes()?;
                            let capacity = a.count()?;
                            buffer.require_capacity(capacity.host)?;
                            a.u32()?;
                            PreparedFilesystemCall::FinalPathNameByHandle {
                                handle,
                                buffer,
                                capacity,
                            }
                        }
                        FilesystemHostOperation::SetFileTime => {
                            let handle = a.native_handle()?;
                            a.i64()?;
                            let last_access = a.bytes()?;
                            if last_access.len() < FILETIME_BYTES {
                                return trap(
                                    "filesystem FILETIME access operand is shorter than 8 bytes",
                                );
                            }
                            let last_write = a.bytes()?;
                            if last_write.len() < FILETIME_BYTES {
                                return trap(
                                    "filesystem FILETIME write operand is shorter than 8 bytes",
                                );
                            }
                            PreparedFilesystemCall::SetFileTime { handle, last_write }
                        }
                        FilesystemHostOperation::LockFileEx => {
                            let handle = a.native_handle()?;
                            let flags = a.u32()?;
                            a.u32()?;
                            a.u32()?;
                            a.u32()?;
                            a.mutable_byte_input(OVERLAPPED_BYTES)?;
                            PreparedFilesystemCall::LockFileEx { handle, flags }
                        }
                        FilesystemHostOperation::UnlockFile => {
                            let handle = a.native_handle()?;
                            a.u32()?;
                            a.u32()?;
                            a.u32()?;
                            a.u32()?;
                            PreparedFilesystemCall::UnlockFile { handle }
                        }
                        FilesystemHostOperation::GetLastError => {
                            PreparedFilesystemCall::GetLastError
                        }
                        FilesystemHostOperation::RemoveName => PreparedFilesystemCall::RemoveName {
                            path: PreparedFilesystemPath::Native(a.path_like_bytes()?),
                        },
                        FilesystemHostOperation::RemoveDirName => {
                            PreparedFilesystemCall::RemoveDirName {
                                path: PreparedFilesystemPath::Native(a.path_like_bytes()?),
                            }
                        }
                        FilesystemHostOperation::ReadMetadata => {
                            let path = a.path()?;
                            let buffer = a.mutable_bytes()?;
                            buffer.require_capacity(STAT_OUTPUT_BYTES)?;
                            PreparedFilesystemCall::ReadMetadata { path, buffer }
                        }
                        FilesystemHostOperation::ReadFileMetadata => {
                            let fd = a.descriptor()?;
                            let buffer = a.mutable_bytes()?;
                            buffer.require_capacity(STAT_OUTPUT_BYTES)?;
                            PreparedFilesystemCall::ReadFileMetadata { fd, buffer }
                        }
                        FilesystemHostOperation::ReadSymlinkMetadata => {
                            let path = a.path()?;
                            let buffer = a.mutable_bytes()?;
                            buffer.require_capacity(STAT_OUTPUT_BYTES)?;
                            PreparedFilesystemCall::ReadSymlinkMetadata { path, buffer }
                        }
                        FilesystemHostOperation::SetLen => PreparedFilesystemCall::SetLen {
                            fd: a.descriptor()?,
                            length: a.i64()?,
                        },
                        FilesystemHostOperation::SetFileTimes => {
                            PreparedFilesystemCall::SetFileTimes {
                                fd: a.descriptor()?,
                                times: a.mutable_byte_input(TIMESPEC_PAIR_BYTES)?,
                            }
                        }
                        FilesystemHostOperation::Sync => PreparedFilesystemCall::Sync {
                            fd: a.descriptor()?,
                        },
                        FilesystemHostOperation::SyncData => PreparedFilesystemCall::SyncData {
                            fd: a.descriptor()?,
                        },
                        FilesystemHostOperation::Duplicate => PreparedFilesystemCall::Duplicate {
                            fd: a.descriptor()?,
                        },
                        FilesystemHostOperation::LockFile => PreparedFilesystemCall::LockFile {
                            fd: a.descriptor()?,
                            operation: a.i32()?,
                        },
                        FilesystemHostOperation::ChangeOwner => {
                            PreparedFilesystemCall::ChangeOwner {
                                path: a.path()?,
                                uid: a.i32()?,
                                gid: a.i32()?,
                            }
                        }
                        FilesystemHostOperation::ChangeOwnerNoFollow => {
                            PreparedFilesystemCall::ChangeOwnerNoFollow {
                                path: a.path()?,
                                uid: a.i32()?,
                                gid: a.i32()?,
                            }
                        }
                        FilesystemHostOperation::ChangeFileOwner => {
                            PreparedFilesystemCall::ChangeFileOwner {
                                fd: a.descriptor()?,
                                uid: a.i32()?,
                                gid: a.i32()?,
                            }
                        }
                        FilesystemHostOperation::Errno => PreparedFilesystemCall::Errno,
                    };
                    a.finish()?;
                    Ok(call)
                }
            }
        }

        pub(crate) mod prepared_outputs {
            use crate::interpreter::evaluator::filesystem::preparation::check_byte_len;
            use crate::interpreter::evaluator::{Cell, EvalResult, Halt, Value, trap};

            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) struct PreparedTransferCount {
                pub(crate) host: usize,
            }

            #[derive(Clone)]
            pub(crate) enum PreparedByteOutput {
                Text {
                    text: crate::value::TextBuffer,
                    capacity: usize,
                },
                Array(Vec<Cell>),
            }

            impl PreparedByteOutput {
                pub(crate) fn capacity(&self) -> usize {
                    match self {
                        Self::Text { capacity, .. } => *capacity,
                        Self::Array(cells) => cells.len(),
                    }
                }

                pub(crate) fn require_capacity(&self, required: usize) -> EvalResult<()> {
                    let capacity = self.capacity();
                    if required > capacity {
                        return trap(format!(
                            "filesystem output requires {required} bytes but the prepared buffer holds {capacity}"
                        ));
                    }
                    Ok(())
                }

                pub(crate) fn snapshot(&self) -> EvalResult<Vec<u8>> {
                    match self {
                        Self::Text { text, .. } => Ok(text.borrow().to_vec()),
                        Self::Array(cells) => cells.iter().map(prepared_byte).collect(),
                    }
                }

                pub(crate) fn validate_contents(&self) -> EvalResult<()> {
                    if let Self::Array(cells) = self {
                        for cell in cells {
                            prepared_byte(cell)?;
                        }
                    }
                    Ok(())
                }

                pub(crate) fn write(&self, bytes: &[u8]) -> EvalResult<()> {
                    check_byte_len(bytes.len())?;
                    self.require_capacity(bytes.len())?;
                    match self {
                        Self::Text { text, .. } => text.write_prefix(bytes).map_err(Halt::Trap)?,
                        Self::Array(cells) => {
                            for (slot, byte) in cells.iter().zip(bytes.iter()) {
                                *slot.borrow_mut() = Value::Int(i64::from(*byte));
                            }
                        }
                    }
                    Ok(())
                }
            }

            pub(crate) fn prepared_byte(cell: &Cell) -> EvalResult<u8> {
                let value = cell.borrow();
                let Value::Int(raw) = *value else {
                    return trap("filesystem byte-array operand contains a non-integer element");
                };
                u8::try_from(raw).map_err(|_| {
                    Halt::Trap(format!(
                        "filesystem byte-array operand element `{raw}` is outside u8 range"
                    ))
                })
            }

            pub(crate) struct PreparedMutableByteInput {
                pub(crate) bytes: Vec<u8>,
            }

            #[derive(Clone)]
            pub(crate) struct PreparedI64Output {
                pub(crate) cell: Cell,
                pub(crate) initial: i64,
            }

            impl PreparedI64Output {
                pub(crate) fn validate_contents(&self) -> EvalResult<()> {
                    if self.cell.borrow().as_int().is_none() {
                        return trap("filesystem mutable scalar became non-integer");
                    }
                    Ok(())
                }

                pub(crate) fn write(&self, value: i64) -> EvalResult<()> {
                    *self.cell.borrow_mut() = Value::Int(value);
                    Ok(())
                }
            }
        }
    }

    pub(crate) mod virtual_filesystem {
        use crate::interpreter::evaluator::{
            EvalResult, Evaluator, FIND_DATA_OUTPUT_BYTES, PreparedByteOutput, VIRTUAL_GID,
            VIRTUAL_UID, VirtualFd, checked_directory_name_snapshot_total,
            checked_directory_record_snapshot_total, pack_dirent_records,
            portable_directory_entry_name,
        };
        use crate::{FilesystemMetadataObservation, FilesystemMetadataObservationKind};

        fn immediate_virtual_child<'path>(path: &'path [u8], prefix: &[u8]) -> Option<&'path [u8]> {
            let rest = path.strip_prefix(prefix)?;
            if rest.is_empty() || rest.contains(&b'/') {
                None
            } else {
                Some(rest)
            }
        }

        impl<'program> Evaluator<'program> {
            pub(crate) fn errno_result(&mut self, errno: i32, result: i64) -> i64 {
                self.virtual_errno = errno;
                result
            }

            pub(crate) fn virtual_at_path(&self, dirfd: i32, name: &[u8]) -> Option<Vec<u8>> {
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

            pub(crate) fn build_find_entries(
                &self,
                dir_path: &[u8],
            ) -> EvalResult<std::collections::VecDeque<(Vec<u8>, bool)>> {
                let mut entries: std::collections::VecDeque<(Vec<u8>, bool)> =
                    std::collections::VecDeque::from([
                        (b".".to_vec(), true),
                        (b"..".to_vec(), true),
                    ]);
                let mut name_bytes = checked_directory_name_snapshot_total(0, 1)?;
                name_bytes = checked_directory_name_snapshot_total(name_bytes, 2)?;
                let mut prefix = dir_path.to_vec();
                prefix.push(b'/');
                for path in self.virtual_files.keys() {
                    if let Some(name) = immediate_virtual_child(path, &prefix) {
                        let name = portable_directory_entry_name(name);
                        name_bytes = checked_directory_name_snapshot_total(name_bytes, name.len())?;
                        entries.push_back((name.to_vec(), false));
                    }
                }
                for path in &self.virtual_dirs {
                    if let Some(name) = immediate_virtual_child(path, &prefix) {
                        let name = portable_directory_entry_name(name);
                        name_bytes = checked_directory_name_snapshot_total(name_bytes, name.len())?;
                        entries.push_back((name.to_vec(), true));
                    }
                }
                Ok(entries)
            }

            pub(crate) fn write_find_data(
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

            pub(crate) fn build_dirent_records(&self, dir_path: &[u8]) -> EvalResult<Vec<u8>> {
                let mut entries: Vec<(Vec<u8>, u8)> = vec![(b".".to_vec(), 4), (b"..".to_vec(), 4)];
                let mut record_bytes = checked_directory_record_snapshot_total(0, 1)?;
                record_bytes = checked_directory_record_snapshot_total(record_bytes, 2)?;
                let mut prefix = dir_path.to_vec();
                prefix.push(b'/');
                for path in self.virtual_files.keys() {
                    if let Some(name) = immediate_virtual_child(path, &prefix) {
                        let name = portable_directory_entry_name(name);
                        record_bytes =
                            checked_directory_record_snapshot_total(record_bytes, name.len())?;
                        entries.push((name.to_vec(), 8));
                    }
                }
                for path in &self.virtual_dirs {
                    if let Some(name) = immediate_virtual_child(path, &prefix) {
                        let name = portable_directory_entry_name(name);
                        record_bytes =
                            checked_directory_record_snapshot_total(record_bytes, name.len())?;
                        entries.push((name.to_vec(), 4));
                    }
                }
                pack_dirent_records(&entries)
            }

            pub(crate) fn write_fs_stat(
                &mut self,
                output: &PreparedByteOutput,
                kind: FilesystemMetadataObservationKind,
                mode: u32,
                size: i64,
                mtime_secs: i64,
            ) -> EvalResult<()> {
                let observation =
                    FilesystemMetadataObservation::new(1, kind, mode, size, mtime_secs);
                let carrier = self.canonical_metadata_carrier(
                    observation,
                    output.capacity(),
                    "metadata output",
                )?;
                output.write(&carrier)?;
                Ok(())
            }

            pub(crate) fn virtual_open(
                &mut self,
                path: Vec<u8>,
                writable: bool,
                create: bool,
            ) -> i32 {
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

            pub(crate) fn virtual_write(&mut self, fd: i32, bytes: &[u8]) -> Option<usize> {
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

            pub(crate) fn virtual_read_n(&mut self, fd: i32, count: usize) -> Option<Vec<u8>> {
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

            pub(crate) fn virtual_read_at(
                &mut self,
                fd: i32,
                offset: i64,
                count: usize,
            ) -> Option<Vec<u8>> {
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

            pub(crate) fn virtual_write_at(
                &mut self,
                fd: i32,
                offset: i64,
                bytes: &[u8],
            ) -> Option<usize> {
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

            pub(crate) fn virtual_hard_link_file(
                &mut self,
                existing: &[u8],
                link: Vec<u8>,
            ) -> bool {
                let Some(content) = self.virtual_files.get(existing).cloned() else {
                    return false;
                };
                let permissions = self.virtual_perms.get(existing).copied();
                let modification_time = self.virtual_times.get(existing).copied();

                self.virtual_files.insert(link.clone(), content);
                match permissions {
                    Some(mode) => {
                        self.virtual_perms.insert(link.clone(), mode);
                    }
                    None => {
                        self.virtual_perms.remove(&link);
                    }
                }
                match modification_time {
                    Some(time) => {
                        self.virtual_times.insert(link, time);
                    }
                    None => {
                        self.virtual_times.remove(&link);
                    }
                }
                true
            }

            pub(crate) fn virtual_open_flags(&mut self, path: Vec<u8>, flags: i32) -> i32 {
                let path = self.virtual_symlinks.get(&path).cloned().unwrap_or(path);
                let exists = self.virtual_files.contains_key(&path);
                let o_creat =
                    crate::interpreter::evaluator::filesystem::host_open_flags::o_creat(flags);
                let o_trunc =
                    crate::interpreter::evaluator::filesystem::host_open_flags::o_trunc(flags);
                let o_append =
                    crate::interpreter::evaluator::filesystem::host_open_flags::o_append(flags);
                let writable = flags & 0x3 != 0;
                if self.virtual_dirs.contains(&path) && writable {
                    self.virtual_errno = 21;
                    return -1;
                }
                let needed_bit = if writable { 0o200 } else { 0o400 };
                if self
                    .virtual_perms
                    .get(&path)
                    .is_some_and(|mode| mode & needed_bit == 0)
                {
                    self.virtual_errno = 13;
                    return -1;
                }
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
                    self.virtual_errno = 2;
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

            pub(crate) fn virtual_seek(
                &mut self,
                fd: i32,
                offset: i64,
                whence: i32,
            ) -> Option<i64> {
                let descriptor = self.virtual_fds.get(&fd)?;
                let path = descriptor.path.clone();
                let cursor = descriptor.cursor.get() as i64;
                let len = self.virtual_files.get(&path).map_or(0, Vec::len) as i64;
                let new_pos = match whence {
                    0 => offset,
                    1 => cursor + offset,
                    2 => len + offset,
                    _ => return None,
                };
                if new_pos < 0 {
                    return None;
                }
                descriptor.cursor.set(new_pos as usize);
                Some(new_pos)
            }

            pub(crate) fn virtual_set_len(&mut self, fd: i32, length: i64) -> i64 {
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

            pub(crate) fn virtual_chown_result(&mut self, uid: i32, gid: i32) -> i64 {
                let effective_uid = if uid == -1 { VIRTUAL_UID as i32 } else { uid };
                let effective_gid = if gid == -1 { VIRTUAL_GID as i32 } else { gid };
                if effective_uid == VIRTUAL_UID as i32 && effective_gid == VIRTUAL_GID as i32 {
                    0
                } else {
                    self.errno_result(1, -1)
                }
            }
        }
    }
}
