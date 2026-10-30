//! The prepared filesystem call and the argument cursor that builds it.

use crate::FilesystemRootedPathOperandResolution;
use crate::interpreter::evaluator::filesystem::preparation::MAX_FILESYSTEM_TRANSFER_BYTES;
use crate::interpreter::evaluator::filesystem::preparation::logical_handle_plans::{
    FilesystemLogicalHandleResultSuccess, FilesystemLogicalHandleRetirementSuccess,
    PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandleOutput,
    PreparedFilesystemLogicalHandlePlan, PreparedFilesystemLogicalHandleRetirement,
};
use crate::interpreter::evaluator::filesystem::preparation::prepared_outputs::{
    PreparedByteOutput, PreparedI64Output, PreparedMutableByteInput, PreparedTransferCount,
    prepared_byte,
};
use crate::interpreter::evaluator::filesystem::preparation::{
    FILETIME_BYTES, FIND_DATA_OUTPUT_BYTES, FilesystemTransferCountError, OVERLAPPED_BYTES,
    PATH_MAX_OUTPUT_BYTES, STAT_OUTPUT_BYTES, TIMESPEC_PAIR_BYTES, check_byte_len,
    check_filesystem_arity, checked_filesystem_transfer_count, checked_relative_component,
    rooted_package_build_operation_refusal,
};
use crate::interpreter::evaluator::{
    EvalResult, Evaluator, ExpressionHandle, FilesystemHostOperation, FilesystemLogicalHandleKind,
    Frame, Halt, Value, rooted_build_path_parts, trap, unsupported, validate_build_relative_path,
};

/// A prepared path retains captured root custody all the way to provider
/// dispatch. Native paths alone contain physical spellings.
pub(crate) enum PreparedFilesystemPath {
    Native(Vec<u8>),
    Captured {
        root: crate::FilesystemGrantRootIdentity,
        relative: Vec<u8>,
    },
}

impl PreparedFilesystemPath {
    pub(crate) fn captured(&self) -> Option<(crate::FilesystemGrantRootIdentity, &[u8])> {
        match self {
            Self::Native(_) => None,
            Self::Captured { root, relative } => Some((*root, relative)),
        }
    }

    pub(crate) fn into_native(self) -> EvalResult<Vec<u8>> {
        match self {
            Self::Native(bytes) => Ok(bytes),
            Self::Captured { .. } => trap("captured input path cannot enter a native provider"),
        }
    }
}

impl std::ops::Deref for PreparedFilesystemPath {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Native(bytes) => bytes,
            Self::Captured { relative, .. } => relative,
        }
    }
}

/// Every canonical authored operand is consumed during preparation, including
/// ABI-shape operands a modeled provider does not otherwise need; only the
/// operands an execution provider reads are retained here.
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
    /// Closed path projection: adding a path operation requires explicitly
    /// assigning its provider custody here, not slipping through native I/O.
    pub(crate) fn has_captured_path(&self) -> bool {
        match self {
            Self::Create { path, .. }
            | Self::Open { path, .. }
            | Self::OpenCreate { path, .. }
            | Self::Remove { path }
            | Self::CreateDir { path, .. }
            | Self::RemoveDir { path }
            | Self::SetPermissions { path, .. }
            | Self::ReadLink { path, .. }
            | Self::Canonicalize { path, .. }
            | Self::OpenPathHandle { path, .. }
            | Self::RemoveName { path }
            | Self::RemoveDirName { path }
            | Self::ReadMetadata { path, .. }
            | Self::ReadSymlinkMetadata { path, .. }
            | Self::ChangeOwner { path, .. }
            | Self::ChangeOwnerNoFollow { path, .. }
            | Self::CreateDirName { name: path, .. }
            | Self::Symlink { link: path, .. } => path.captured().is_some(),
            Self::Rename { from, to } => from.captured().is_some() || to.captured().is_some(),
            Self::HardLink { original, link } => {
                original.captured().is_some() || link.captured().is_some()
            }
            Self::CreateHardLink { link, existing, .. } => {
                link.captured().is_some() || existing.captured().is_some()
            }
            Self::Read { .. }
            | Self::Write { .. }
            | Self::ReadAt { .. }
            | Self::WriteAt { .. }
            | Self::Close { .. }
            | Self::Seek { .. }
            | Self::OpenAt { .. }
            | Self::UnlinkAt { .. }
            | Self::SetFilePermissions { .. }
            | Self::ReadDir { .. }
            | Self::FindFirst { .. }
            | Self::FindNext { .. }
            | Self::FindClose { .. }
            | Self::CloseHandle { .. }
            | Self::GetOsfHandle { .. }
            | Self::FinalPathNameByHandle { .. }
            | Self::SetFileTime { .. }
            | Self::LockFileEx { .. }
            | Self::UnlockFile { .. }
            | Self::GetLastError
            | Self::ReadFileMetadata { .. }
            | Self::SetLen { .. }
            | Self::SetFileTimes { .. }
            | Self::Sync { .. }
            | Self::SyncData { .. }
            | Self::Duplicate { .. }
            | Self::LockFile { .. }
            | Self::ChangeFileOwner { .. }
            | Self::Errno => false,
        }
    }

    /// Later arguments can alias earlier carriers. Validate their current
    /// contents without retaining another copy before entering the provider.
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
            Self::FindFirst { data, .. } | Self::FindNext { data, .. } => data.validate_contents(),
            _ => Ok(()),
        }
    }

    /// Project the closed canonical call into descriptor/handle roles before a
    /// provider consumes it. Scalar values that merely share an integer ABI
    /// width (pointers, offsets, flags, ownership IDs) never enter this plan.
    pub(crate) fn logical_handle_plan(&self) -> PreparedFilesystemLogicalHandlePlan {
        use FilesystemLogicalHandleKind as Kind;
        use FilesystemLogicalHandleResultSuccess as ResultSuccess;
        use FilesystemLogicalHandleRetirementSuccess as RetireSuccess;

        let input =
            |operand_ordinal, kind, raw, null_allowed| PreparedFilesystemLogicalHandleInput {
                operand_ordinal,
                kind,
                raw,
                null_allowed,
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
            Halt::Trap("canonical filesystem call is missing an authored operand".to_owned())
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
        .ok_or_else(|| Halt::Trap("filesystem call file handle is not an fd".to_owned()))?;
        let descriptor = i32::try_from(raw)
            .map_err(|_| Halt::Trap(format!("filesystem fd `{raw}` is out of range")))?;
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
            Value::Ref(reference) if reference.storage.is_none() => match &*reference.cell.borrow()
            {
                Value::Array(cells) => {
                    check_byte_len(cells.len())?;
                    cells.iter().map(prepared_byte).collect()
                }
                other => unsupported(format!(
                    "filesystem call expected byte data behind a reference, got {other:?}"
                )),
            },
            other => unsupported(format!("filesystem call expected byte data, got {other:?}")),
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
            if self.evaluator.rooted_build_paths_required
                && self
                    .evaluator
                    .real_fs
                    .as_ref()
                    .is_some_and(crate::interpreter::evaluator::filesystem::real::RealFs::is_scoped)
            {
                return Err(Halt::Trap(
                    "package build filesystem paths must come from BuildSource::resolve or BuildOutput::resolve"
                        .to_owned(),
                ));
            }
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
        let provider_path = {
            let filesystem = self.evaluator.real_fs.as_ref().ok_or_else(|| {
                Halt::Trap("rooted build path requires a scoped real filesystem".to_owned())
            })?;
            filesystem
                .prepare_rooted_path(root, &relative)
                .ok_or_else(|| {
                    Halt::Trap("rooted build path names no compiler-supplied grant root".to_owned())
                })?
        };
        let resolution =
            FilesystemRootedPathOperandResolution::new(operand_ordinal, root, relative);
        self.evaluator
            .record_prepared_filesystem_rooted_path_operand_resolution(
                self.attempt_index,
                resolution,
            )?;
        Ok(provider_path)
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
        let mut a = FilesystemArgumentCursor::new(self, attempt_index, arguments, frame);
        let call = match operation {
            FilesystemHostOperation::Create => {
                let path = a.path()?;
                // Canonical create mode: validated and consumed, not retained.
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
            FilesystemHostOperation::Remove => PreparedFilesystemCall::Remove { path: a.path()? },
            FilesystemHostOperation::Seek => PreparedFilesystemCall::Seek {
                fd: a.descriptor()?,
                offset: a.i64()?,
                whence: a.i32()?,
            },
            FilesystemHostOperation::CreateDir => {
                let path = a.path()?;
                // Canonical create mode: validated and consumed, not retained.
                a.i32()?;
                PreparedFilesystemCall::CreateDir { path }
            }
            FilesystemHostOperation::RemoveDir => {
                PreparedFilesystemCall::RemoveDir { path: a.path()? }
            }
            FilesystemHostOperation::CreateDirName => {
                let name = PreparedFilesystemPath::Native(a.path_like_bytes()?);
                // Canonical create mode: validated and consumed, not retained.
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
            FilesystemHostOperation::SetPermissions => PreparedFilesystemCall::SetPermissions {
                path: a.path()?,
                mode: a.u32()?,
            },
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
                // Windows security-attributes operand: consumed, not retained.
                a.i64()?;
                PreparedFilesystemCall::CreateHardLink { link, existing }
            }
            FilesystemHostOperation::OpenPathHandle => {
                let path = a.path()?;
                // CreateFileA's access/share/security/disposition/attribute
                // operands are ABI shape: consumed here, never read back.
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
            FilesystemHostOperation::CloseHandle => PreparedFilesystemCall::CloseHandle {
                handle: a.native_handle()?,
            },
            FilesystemHostOperation::GetOsfHandle => PreparedFilesystemCall::GetOsfHandle {
                fd: a.descriptor()?,
            },
            FilesystemHostOperation::FinalPathNameByHandle => {
                let handle = a.native_handle()?;
                let buffer = a.mutable_bytes()?;
                let capacity = a.count()?;
                buffer.require_capacity(capacity.host)?;
                // Win32 flags operand: consumed, not retained.
                a.u32()?;
                PreparedFilesystemCall::FinalPathNameByHandle {
                    handle,
                    buffer,
                    capacity,
                }
            }
            FilesystemHostOperation::SetFileTime => {
                let handle = a.native_handle()?;
                // Creation-time operand: consumed, not retained.
                a.i64()?;
                let last_access = a.bytes()?;
                if last_access.len() < FILETIME_BYTES {
                    return trap("filesystem FILETIME access operand is shorter than 8 bytes");
                }
                let last_write = a.bytes()?;
                if last_write.len() < FILETIME_BYTES {
                    return trap("filesystem FILETIME write operand is shorter than 8 bytes");
                }
                PreparedFilesystemCall::SetFileTime { handle, last_write }
            }
            FilesystemHostOperation::LockFileEx => {
                let handle = a.native_handle()?;
                let flags = a.u32()?;
                // reserved/length/OVERLAPPED operands are ABI shape: consumed
                // (including the OVERLAPPED capacity check), not retained.
                a.u32()?;
                a.u32()?;
                a.u32()?;
                a.mutable_byte_input(OVERLAPPED_BYTES)?;
                PreparedFilesystemCall::LockFileEx { handle, flags }
            }
            FilesystemHostOperation::UnlockFile => {
                let handle = a.native_handle()?;
                // Byte-range operands are ABI shape: consumed, not retained.
                a.u32()?;
                a.u32()?;
                a.u32()?;
                a.u32()?;
                PreparedFilesystemCall::UnlockFile { handle }
            }
            FilesystemHostOperation::GetLastError => PreparedFilesystemCall::GetLastError,
            FilesystemHostOperation::RemoveName => PreparedFilesystemCall::RemoveName {
                path: PreparedFilesystemPath::Native(a.path_like_bytes()?),
            },
            FilesystemHostOperation::RemoveDirName => PreparedFilesystemCall::RemoveDirName {
                path: PreparedFilesystemPath::Native(a.path_like_bytes()?),
            },
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
            FilesystemHostOperation::SetFileTimes => PreparedFilesystemCall::SetFileTimes {
                fd: a.descriptor()?,
                times: a.mutable_byte_input(TIMESPEC_PAIR_BYTES)?,
            },
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
            FilesystemHostOperation::ChangeOwner => PreparedFilesystemCall::ChangeOwner {
                path: a.path()?,
                uid: a.i32()?,
                gid: a.i32()?,
            },
            FilesystemHostOperation::ChangeOwnerNoFollow => {
                PreparedFilesystemCall::ChangeOwnerNoFollow {
                    path: a.path()?,
                    uid: a.i32()?,
                    gid: a.i32()?,
                }
            }
            FilesystemHostOperation::ChangeFileOwner => PreparedFilesystemCall::ChangeFileOwner {
                fd: a.descriptor()?,
                uid: a.i32()?,
                gid: a.i32()?,
            },
            FilesystemHostOperation::Errno => PreparedFilesystemCall::Errno,
        };
        a.finish()?;
        Ok(call)
    }
}
