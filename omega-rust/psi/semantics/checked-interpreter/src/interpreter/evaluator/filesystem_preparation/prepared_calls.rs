//! The prepared filesystem call and the argument cursor that builds it.

use crate::interpreter::evaluator::filesystem_preparation::MAX_FILESYSTEM_TRANSFER_BYTES;
use crate::interpreter::evaluator::filesystem_preparation::logical_handle_plans::{
    FilesystemLogicalHandleResultSuccess, FilesystemLogicalHandleRetirementSuccess,
    PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandleOutput,
    PreparedFilesystemLogicalHandlePlan, PreparedFilesystemLogicalHandleRetirement,
    PreparedFilesystemPreparation,
};
use crate::interpreter::evaluator::filesystem_preparation::observation_plans::{
    PreparedFilesystemMutableObservationPlan, mutable_byte_observation, mutable_i64_observation,
    observed_bytes, observed_path_like_bytes, observed_scalar,
};
use crate::interpreter::evaluator::filesystem_preparation::prepared_outputs::{
    PreparedByteOutput, PreparedI64Output, PreparedMutableByteInput, PreparedTransferCount,
    prepared_byte,
};
use crate::interpreter::evaluator::filesystem_preparation::{
    FILETIME_BYTES, FIND_DATA_OUTPUT_BYTES, FilesystemTransferCountError, OVERLAPPED_BYTES,
    PATH_MAX_OUTPUT_BYTES, STAT_OUTPUT_BYTES, TIMESPEC_PAIR_BYTES, check_byte_len,
    check_filesystem_arity, checked_filesystem_transfer_count, checked_relative_component,
    rooted_package_build_operation_refusal,
};
use crate::interpreter::evaluator::{
    EvalResult, Evaluator, ExpressionHandle, FilesystemHostOperation, FilesystemLogicalHandleKind,
    Frame, Halt, Value, real_filesystem, rooted_build_path_parts, trap, unsupported,
    validate_build_relative_path,
};
use crate::{
    FilesystemByteOperand, FilesystemPathLikeOperand, FilesystemRootedPathOperandResolution,
    FilesystemScalarOperand, FilesystemScalarOperandValue,
};

/// Every canonical authored operand is represented, including ABI-shape
/// operands a modeled provider does not otherwise need.
#[allow(dead_code)]
pub(crate) enum PreparedFilesystemCall {
    Create {
        path: Vec<u8>,
        mode: i32,
    },
    Open {
        path: Vec<u8>,
        flags: i32,
    },
    OpenCreate {
        path: Vec<u8>,
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
        path: Vec<u8>,
    },
    Seek {
        fd: i32,
        offset: i64,
        whence: i32,
    },
    CreateDir {
        path: Vec<u8>,
        mode: i32,
    },
    RemoveDir {
        path: Vec<u8>,
    },
    CreateDirName {
        name: Vec<u8>,
        mode: i32,
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
        path: Vec<u8>,
        mode: u32,
    },
    SetFilePermissions {
        fd: i32,
        mode: u32,
    },
    Rename {
        from: Vec<u8>,
        to: Vec<u8>,
    },
    HardLink {
        original: Vec<u8>,
        link: Vec<u8>,
    },
    Symlink {
        target: Vec<u8>,
        link: Vec<u8>,
    },
    ReadLink {
        path: Vec<u8>,
        buffer: PreparedByteOutput,
        count: PreparedTransferCount,
    },
    Canonicalize {
        path: Vec<u8>,
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
        link: Vec<u8>,
        existing: Vec<u8>,
        security_attributes: i64,
    },
    OpenPathHandle {
        path: Vec<u8>,
        desired_access: u32,
        share_mode: u32,
        security_attributes: i64,
        creation_disposition: u32,
        flags_and_attributes: u32,
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
        flags: u32,
    },
    SetFileTime {
        handle: i64,
        creation: i64,
        last_access: Vec<u8>,
        last_write: Vec<u8>,
    },
    LockFileEx {
        handle: i64,
        flags: u32,
        reserved: u32,
        length_low: u32,
        length_high: u32,
        overlapped: PreparedMutableByteInput,
    },
    UnlockFile {
        handle: i64,
        offset_low: u32,
        offset_high: u32,
        length_low: u32,
        length_high: u32,
    },
    GetLastError,
    RemoveName {
        path: Vec<u8>,
    },
    RemoveDirName {
        path: Vec<u8>,
    },
    ReadMetadata {
        path: Vec<u8>,
        buffer: PreparedByteOutput,
    },
    ReadFileMetadata {
        fd: i32,
        buffer: PreparedByteOutput,
    },
    ReadSymlinkMetadata {
        path: Vec<u8>,
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
        path: Vec<u8>,
        uid: i32,
        gid: i32,
    },
    ChangeOwnerNoFollow {
        path: Vec<u8>,
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

    /// Project canonical non-handle scalars, immutable payload bytes, and
    /// unrooted path-like spellings from a fully prepared call. Rooted paths
    /// remain excluded because their separate semantic sidecar owns portable
    /// input resolution and scoped grant evidence owns authorization; neither
    /// may be reconstructed from raw physical provider spellings.
    pub(crate) fn operand_observation_plan(
        &self,
    ) -> (
        Vec<FilesystemScalarOperand>,
        Vec<FilesystemByteOperand>,
        Vec<FilesystemPathLikeOperand>,
    ) {
        use FilesystemScalarOperandValue as Scalar;

        let mut scalars = Vec::new();
        let mut bytes = Vec::new();
        let mut path_like = Vec::new();

        match self {
            Self::Create { mode, .. } => scalars.push(observed_scalar(1, Scalar::I32(*mode))),
            Self::Open { flags, .. } => scalars.push(observed_scalar(1, Scalar::I32(*flags))),
            Self::OpenCreate { flags, mode, .. } => {
                scalars.push(observed_scalar(1, Scalar::I32(*flags)));
                scalars.push(observed_scalar(2, Scalar::I32(*mode)));
            }
            Self::Read { count, .. } => {
                scalars.push(observed_scalar(2, Scalar::U64(count.raw)));
            }
            Self::Write { bytes: value, .. } => bytes.push(observed_bytes(1, value)),
            Self::ReadAt { count, offset, .. } => {
                scalars.push(observed_scalar(2, Scalar::U64(count.raw)));
                scalars.push(observed_scalar(3, Scalar::I64(*offset)));
            }
            Self::WriteAt {
                bytes: value,
                offset,
                ..
            } => {
                bytes.push(observed_bytes(1, value));
                scalars.push(observed_scalar(2, Scalar::I64(*offset)));
            }
            Self::Seek { offset, whence, .. } => {
                scalars.push(observed_scalar(1, Scalar::I64(*offset)));
                scalars.push(observed_scalar(2, Scalar::I32(*whence)));
            }
            Self::CreateDir { mode, .. } => {
                scalars.push(observed_scalar(1, Scalar::I32(*mode)));
            }
            Self::CreateDirName { name, mode } => {
                path_like.push(observed_path_like_bytes(0, name));
                scalars.push(observed_scalar(1, Scalar::I32(*mode)));
            }
            Self::OpenAt { name, flags, .. } | Self::UnlinkAt { name, flags, .. } => {
                bytes.push(observed_bytes(1, name));
                scalars.push(observed_scalar(2, Scalar::I32(*flags)));
            }
            Self::SetPermissions { mode, .. } | Self::SetFilePermissions { mode, .. } => {
                scalars.push(observed_scalar(1, Scalar::U32(*mode)));
            }
            Self::ReadLink { count, .. } | Self::ReadDir { count, .. } => {
                scalars.push(observed_scalar(2, Scalar::U64(count.raw)));
            }
            Self::CreateHardLink {
                security_attributes,
                ..
            } => scalars.push(observed_scalar(2, Scalar::I64(*security_attributes))),
            Self::OpenPathHandle {
                desired_access,
                share_mode,
                security_attributes,
                creation_disposition,
                flags_and_attributes,
                ..
            } => {
                scalars.push(observed_scalar(1, Scalar::U32(*desired_access)));
                scalars.push(observed_scalar(2, Scalar::U32(*share_mode)));
                scalars.push(observed_scalar(3, Scalar::I64(*security_attributes)));
                scalars.push(observed_scalar(4, Scalar::U32(*creation_disposition)));
                scalars.push(observed_scalar(5, Scalar::U32(*flags_and_attributes)));
            }
            Self::FinalPathNameByHandle {
                capacity, flags, ..
            } => {
                scalars.push(observed_scalar(2, Scalar::U64(capacity.raw)));
                scalars.push(observed_scalar(3, Scalar::U32(*flags)));
            }
            Self::SetFileTime {
                creation,
                last_access,
                last_write,
                ..
            } => {
                scalars.push(observed_scalar(1, Scalar::I64(*creation)));
                bytes.push(observed_bytes(2, last_access));
                bytes.push(observed_bytes(3, last_write));
            }
            Self::LockFileEx {
                flags,
                reserved,
                length_low,
                length_high,
                ..
            } => {
                scalars.push(observed_scalar(1, Scalar::U32(*flags)));
                scalars.push(observed_scalar(2, Scalar::U32(*reserved)));
                scalars.push(observed_scalar(3, Scalar::U32(*length_low)));
                scalars.push(observed_scalar(4, Scalar::U32(*length_high)));
            }
            Self::UnlockFile {
                offset_low,
                offset_high,
                length_low,
                length_high,
                ..
            } => {
                scalars.push(observed_scalar(1, Scalar::U32(*offset_low)));
                scalars.push(observed_scalar(2, Scalar::U32(*offset_high)));
                scalars.push(observed_scalar(3, Scalar::U32(*length_low)));
                scalars.push(observed_scalar(4, Scalar::U32(*length_high)));
            }
            Self::SetLen { length, .. } => {
                scalars.push(observed_scalar(1, Scalar::I64(*length)));
            }
            Self::LockFile { operation, .. } => {
                scalars.push(observed_scalar(1, Scalar::I32(*operation)));
            }
            Self::ChangeOwner { uid, gid, .. }
            | Self::ChangeOwnerNoFollow { uid, gid, .. }
            | Self::ChangeFileOwner { uid, gid, .. } => {
                scalars.push(observed_scalar(1, Scalar::I32(*uid)));
                scalars.push(observed_scalar(2, Scalar::I32(*gid)));
            }
            Self::Symlink { target, .. }
            | Self::FindFirst {
                pattern: target, ..
            }
            | Self::RemoveName { path: target }
            | Self::RemoveDirName { path: target } => {
                path_like.push(observed_path_like_bytes(0, target));
            }
            Self::Close { .. }
            | Self::Remove { .. }
            | Self::RemoveDir { .. }
            | Self::Rename { .. }
            | Self::HardLink { .. }
            | Self::Canonicalize { .. }
            | Self::FindNext { .. }
            | Self::FindClose { .. }
            | Self::CloseHandle { .. }
            | Self::GetOsfHandle { .. }
            | Self::GetLastError
            | Self::ReadMetadata { .. }
            | Self::ReadFileMetadata { .. }
            | Self::ReadSymlinkMetadata { .. }
            | Self::SetFileTimes { .. }
            | Self::Sync { .. }
            | Self::SyncData { .. }
            | Self::Duplicate { .. }
            | Self::Errno => {}
        }
        (scalars, bytes, path_like)
    }

    /// Snapshot mutable carriers only after all authored arguments have been
    /// evaluated. A later argument may alias an earlier carrier, so capturing
    /// while the argument cursor advances would not describe provider-visible
    /// pre-state.
    pub(crate) fn mutable_observation_plan(
        &self,
    ) -> EvalResult<PreparedFilesystemMutableObservationPlan> {
        let mut byte_operands = Vec::new();
        let mut i64_operands = Vec::new();
        match self {
            Self::Read { buffer, .. }
            | Self::ReadAt { buffer, .. }
            | Self::ReadLink { buffer, .. }
            | Self::Canonicalize { buffer, .. }
            | Self::FinalPathNameByHandle { buffer, .. }
            | Self::ReadMetadata { buffer, .. }
            | Self::ReadFileMetadata { buffer, .. }
            | Self::ReadSymlinkMetadata { buffer, .. } => {
                byte_operands.push(mutable_byte_observation(1, buffer)?);
            }
            Self::ReadDir {
                buffer, position, ..
            } => {
                byte_operands.push(mutable_byte_observation(1, buffer)?);
                i64_operands.push(mutable_i64_observation(3, position)?);
            }
            Self::FindFirst { data, .. } | Self::FindNext { data, .. } => {
                byte_operands.push(mutable_byte_observation(1, data)?);
            }
            Self::LockFileEx { overlapped, .. } => {
                byte_operands.push(mutable_byte_observation(5, &overlapped.output)?);
            }
            Self::SetFileTimes { times, .. } => {
                byte_operands.push(mutable_byte_observation(1, &times.output)?);
            }
            Self::Create { .. }
            | Self::Open { .. }
            | Self::OpenCreate { .. }
            | Self::Write { .. }
            | Self::WriteAt { .. }
            | Self::Close { .. }
            | Self::Remove { .. }
            | Self::Seek { .. }
            | Self::CreateDir { .. }
            | Self::RemoveDir { .. }
            | Self::CreateDirName { .. }
            | Self::OpenAt { .. }
            | Self::UnlinkAt { .. }
            | Self::SetPermissions { .. }
            | Self::SetFilePermissions { .. }
            | Self::Rename { .. }
            | Self::HardLink { .. }
            | Self::Symlink { .. }
            | Self::FindClose { .. }
            | Self::CreateHardLink { .. }
            | Self::OpenPathHandle { .. }
            | Self::CloseHandle { .. }
            | Self::GetOsfHandle { .. }
            | Self::SetFileTime { .. }
            | Self::UnlockFile { .. }
            | Self::GetLastError
            | Self::RemoveName { .. }
            | Self::RemoveDirName { .. }
            | Self::SetLen { .. }
            | Self::Sync { .. }
            | Self::SyncData { .. }
            | Self::Duplicate { .. }
            | Self::LockFile { .. }
            | Self::ChangeOwner { .. }
            | Self::ChangeOwnerNoFollow { .. }
            | Self::ChangeFileOwner { .. }
            | Self::Errno => {}
        }
        Ok(PreparedFilesystemMutableObservationPlan {
            byte_operands,
            i64_operands,
        })
    }
}

struct FilesystemArgumentCursor<'evaluation, 'program, 'arguments, 'frame> {
    evaluator: &'evaluation mut Evaluator<'program>,
    attempt_index: usize,
    arguments: std::slice::Iter<'arguments, ExpressionHandle>,
    frame: &'frame Frame,
    consumed: usize,
    rooted_path_operand_resolutions: Vec<FilesystemRootedPathOperandResolution>,
}

impl<'evaluation, 'program, 'arguments, 'frame>
    FilesystemArgumentCursor<'evaluation, 'program, 'arguments, 'frame>
{
    fn new(
        evaluator: &'evaluation mut Evaluator<'program>,
        attempt_index: usize,
        arguments: &'arguments [ExpressionHandle],
        frame: &'frame Frame,
    ) -> Self {
        Self {
            evaluator,
            attempt_index,
            arguments: arguments.iter(),
            frame,
            consumed: 0,
            rooted_path_operand_resolutions: Vec::new(),
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
        let (operand_ordinal, value) = self.integer()?;
        self.evaluator.record_prepared_filesystem_scalar_operand(
            self.attempt_index,
            observed_scalar(operand_ordinal, FilesystemScalarOperandValue::I64(value)),
        );
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
        let (operand_ordinal, raw) = self.integer()?;
        let value = i32::try_from(raw).map_err(|_| {
            Halt::Trap(format!(
                "canonical filesystem i32 operand `{raw}` is out of range"
            ))
        })?;
        self.evaluator.record_prepared_filesystem_scalar_operand(
            self.attempt_index,
            observed_scalar(operand_ordinal, FilesystemScalarOperandValue::I32(value)),
        );
        Ok(value)
    }

    fn u32(&mut self) -> EvalResult<u32> {
        let (operand_ordinal, raw) = self.integer()?;
        let value = u32::try_from(raw).map_err(|_| {
            Halt::Trap(format!(
                "canonical filesystem u32 operand `{raw}` is out of range"
            ))
        })?;
        self.evaluator.record_prepared_filesystem_scalar_operand(
            self.attempt_index,
            observed_scalar(operand_ordinal, FilesystemScalarOperandValue::U32(value)),
        );
        Ok(value)
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
        let (operand_ordinal, raw) = self.integer()?;
        let typed = u64::try_from(raw).map_err(|_| {
            Halt::Trap("filesystem transfer count is negative or not host-representable".to_owned())
        })?;
        self.evaluator.record_prepared_filesystem_scalar_operand(
            self.attempt_index,
            observed_scalar(operand_ordinal, FilesystemScalarOperandValue::U64(typed)),
        );
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
            Value::Ref(target) => match &*target.borrow() {
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
        let (operand_ordinal, bytes) = self.raw_bytes()?;
        self.evaluator.record_prepared_filesystem_byte_operand(
            self.attempt_index,
            observed_bytes(operand_ordinal, &bytes),
        )?;
        Ok(bytes)
    }

    fn path_like_bytes(&mut self) -> EvalResult<Vec<u8>> {
        let (operand_ordinal, bytes) = self.raw_bytes()?;
        self.evaluator
            .record_prepared_filesystem_path_like_operand(
                self.attempt_index,
                observed_path_like_bytes(operand_ordinal, &bytes),
            )?;
        Ok(bytes)
    }

    fn relative_component(&mut self) -> EvalResult<Vec<u8>> {
        let (operand_ordinal, bytes) = self.raw_bytes()?;
        let bytes = checked_relative_component(bytes)?;
        self.evaluator.record_prepared_filesystem_byte_operand(
            self.attempt_index,
            observed_bytes(operand_ordinal, &bytes),
        )?;
        Ok(bytes)
    }

    fn path(&mut self) -> EvalResult<Vec<u8>> {
        let (operand_ordinal, value) = self.value()?;
        let Some((root, relative)) = rooted_build_path_parts(&value)? else {
            if self.evaluator.rooted_build_paths_required
                && self
                    .evaluator
                    .real_fs
                    .as_ref()
                    .is_some_and(real_filesystem::RealFs::is_scoped)
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
                    Ok(text.to_vec())
                }
                Value::Array(cells) => {
                    check_byte_len(cells.len())?;
                    cells.iter().map(prepared_byte).collect()
                }
                other => unsupported(format!(
                    "filesystem call expected path byte data, got {other:?}"
                )),
            };
        };
        validate_build_relative_path(&relative)?;
        let provider_path = if self.evaluator.filesystem_replay.is_some() {
            let mut stable = format!("/root/{}", root.get()).into_bytes();
            if !relative.is_empty() {
                stable.push(b'/');
                stable.extend_from_slice(&relative);
            }
            stable
        } else {
            let filesystem = self.evaluator.real_fs.as_ref().ok_or_else(|| {
                Halt::Trap("rooted build path requires a scoped real filesystem".to_owned())
            })?;
            filesystem
                .rooted_path_bytes(root, &relative)
                .ok_or_else(|| {
                    Halt::Trap("rooted build path names no compiler-supplied grant root".to_owned())
                })?
        };
        let resolution = FilesystemRootedPathOperandResolution {
            operand_ordinal,
            root,
            relative_path: relative,
        };
        self.evaluator
            .record_prepared_filesystem_rooted_path_operand_resolution(
                self.attempt_index,
                resolution.clone(),
            )?;
        self.rooted_path_operand_resolutions.push(resolution);
        Ok(provider_path)
    }

    fn mutable_bytes(&mut self) -> EvalResult<PreparedByteOutput> {
        let (operand_ordinal, handle) = self.next()?;
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
        self.evaluator
            .record_prepared_filesystem_mutable_byte_operand_resolution(
                self.attempt_index,
                operand_ordinal,
                &output,
            )?;
        Ok(output)
    }

    fn mutable_i64(&mut self) -> EvalResult<PreparedI64Output> {
        let (operand_ordinal, handle) = self.next()?;
        let cell = self.evaluator.resolve_place(handle, self.frame)?;
        let cell = self.evaluator.deref_cell(cell);
        let initial = match *cell.borrow() {
            Value::Int(value) => value,
            _ => return trap("filesystem mutable scalar operand is not an integer"),
        };
        self.evaluator
            .record_prepared_filesystem_mutable_i64_operand_resolution(
                self.attempt_index,
                operand_ordinal,
                initial,
            );
        Ok(PreparedI64Output { cell, initial })
    }

    fn mutable_byte_input(
        &mut self,
        required_bytes: usize,
    ) -> EvalResult<PreparedMutableByteInput> {
        let output = self.mutable_bytes()?;
        output.require_capacity(required_bytes)?;
        let bytes = output.snapshot()?;
        Ok(PreparedMutableByteInput { output, bytes })
    }

    fn finish(self) -> EvalResult<Vec<FilesystemRootedPathOperandResolution>> {
        if self.arguments.len() == 0 {
            Ok(self.rooted_path_operand_resolutions)
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
        frame: &Frame,
    ) -> EvalResult<PreparedFilesystemPreparation> {
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
            FilesystemHostOperation::Create => PreparedFilesystemCall::Create {
                path: a.path()?,
                mode: a.i32()?,
            },
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
            FilesystemHostOperation::CreateDir => PreparedFilesystemCall::CreateDir {
                path: a.path()?,
                mode: a.i32()?,
            },
            FilesystemHostOperation::RemoveDir => {
                PreparedFilesystemCall::RemoveDir { path: a.path()? }
            }
            FilesystemHostOperation::CreateDirName => PreparedFilesystemCall::CreateDirName {
                name: a.path_like_bytes()?,
                mode: a.i32()?,
            },
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
            FilesystemHostOperation::CreateHardLink => PreparedFilesystemCall::CreateHardLink {
                link: a.path()?,
                existing: a.path()?,
                security_attributes: a.i64()?,
            },
            FilesystemHostOperation::OpenPathHandle => PreparedFilesystemCall::OpenPathHandle {
                path: a.path()?,
                desired_access: a.u32()?,
                share_mode: a.u32()?,
                security_attributes: a.i64()?,
                creation_disposition: a.u32()?,
                flags_and_attributes: a.u32()?,
                template_file: a.nullable_native_handle()?,
            },
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
                let flags = a.u32()?;
                PreparedFilesystemCall::FinalPathNameByHandle {
                    handle,
                    buffer,
                    capacity,
                    flags,
                }
            }
            FilesystemHostOperation::SetFileTime => {
                let handle = a.native_handle()?;
                let creation = a.i64()?;
                let last_access = a.bytes()?;
                if last_access.len() < FILETIME_BYTES {
                    return trap("filesystem FILETIME access operand is shorter than 8 bytes");
                }
                let last_write = a.bytes()?;
                if last_write.len() < FILETIME_BYTES {
                    return trap("filesystem FILETIME write operand is shorter than 8 bytes");
                }
                PreparedFilesystemCall::SetFileTime {
                    handle,
                    creation,
                    last_access,
                    last_write,
                }
            }
            FilesystemHostOperation::LockFileEx => PreparedFilesystemCall::LockFileEx {
                handle: a.native_handle()?,
                flags: a.u32()?,
                reserved: a.u32()?,
                length_low: a.u32()?,
                length_high: a.u32()?,
                overlapped: a.mutable_byte_input(OVERLAPPED_BYTES)?,
            },
            FilesystemHostOperation::UnlockFile => PreparedFilesystemCall::UnlockFile {
                handle: a.native_handle()?,
                offset_low: a.u32()?,
                offset_high: a.u32()?,
                length_low: a.u32()?,
                length_high: a.u32()?,
            },
            FilesystemHostOperation::GetLastError => PreparedFilesystemCall::GetLastError,
            FilesystemHostOperation::RemoveName => PreparedFilesystemCall::RemoveName {
                path: a.path_like_bytes()?,
            },
            FilesystemHostOperation::RemoveDirName => PreparedFilesystemCall::RemoveDirName {
                path: a.path_like_bytes()?,
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
        let rooted_path_operand_resolutions = a.finish()?;
        Ok(PreparedFilesystemPreparation {
            call,
            rooted_path_operand_resolutions,
        })
    }
}
