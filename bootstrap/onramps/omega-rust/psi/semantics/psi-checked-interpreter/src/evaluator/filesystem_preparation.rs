use super::*;
use crate::{
    FilesystemByteOperand, FilesystemGrantRootIdentity, FilesystemMutableByteOperand,
    FilesystemMutableI64Operand, FilesystemScalarOperand, FilesystemScalarOperandValue,
};

pub(super) const MAX_FILESYSTEM_TRANSFER_BYTES: usize = 16 * 1024 * 1024;
const FILETIME_BYTES: usize = 8;
const FIND_DATA_OUTPUT_BYTES: usize = 320;
const OVERLAPPED_BYTES: usize = 32;
const PATH_MAX_OUTPUT_BYTES: usize = 1024;
const STAT_OUTPUT_BYTES: usize = 144;
const TIMESPEC_PAIR_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesystemTransferCountError {
    NegativeOrUnrepresentable,
    ExceedsEvaluatorLimit,
}

pub(super) fn checked_filesystem_transfer_count(
    raw: i64,
) -> Result<PreparedTransferCount, FilesystemTransferCountError> {
    let host = usize::try_from(raw)
        .map_err(|_| FilesystemTransferCountError::NegativeOrUnrepresentable)?;
    if host > MAX_FILESYSTEM_TRANSFER_BYTES {
        return Err(FilesystemTransferCountError::ExceedsEvaluatorLimit);
    }
    Ok(PreparedTransferCount {
        raw: raw as u64,
        host,
    })
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

/// Both interpreter providers model Win32 HANDLEs with their i32 descriptor
/// tables. Reject values outside that synthetic domain instead of allowing a
/// lossy cast to alias an unrelated open descriptor.
pub(super) fn synthetic_handle_fd(handle: i64) -> Option<i32> {
    i32::try_from(handle).ok()
}

fn check_filesystem_arity(operation: FilesystemHostOperation, actual: usize) -> EvalResult<()> {
    let expected = operation.operand_kinds().len();
    if actual != expected {
        return trap(format!(
            "canonical filesystem operation `{operation}` expects {expected} operand(s), got {actual}"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreparedTransferCount {
    pub(super) raw: u64,
    pub(super) host: usize,
}

#[derive(Clone)]
pub(super) enum PreparedByteOutput {
    Text {
        text: std::rc::Rc<std::cell::RefCell<Vec<u8>>>,
        capacity: usize,
    },
    Array(Vec<Cell>),
}

impl PreparedByteOutput {
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::Text { capacity, .. } => *capacity,
            Self::Array(cells) => cells.len(),
        }
    }

    pub(super) fn require_capacity(&self, required: usize) -> EvalResult<()> {
        let capacity = self.capacity();
        if required > capacity {
            return trap(format!(
                "filesystem output requires {required} bytes but the prepared buffer holds {capacity}"
            ));
        }
        Ok(())
    }

    pub(super) fn snapshot(&self) -> EvalResult<Vec<u8>> {
        match self {
            Self::Text { text, .. } => Ok(text.borrow().clone()),
            Self::Array(cells) => cells.iter().map(prepared_byte).collect(),
        }
    }

    pub(super) fn write(&self, bytes: &[u8]) -> EvalResult<()> {
        check_byte_len(bytes.len())?;
        self.require_capacity(bytes.len())?;
        match self {
            Self::Text { text, .. } => text.borrow_mut()[..bytes.len()].copy_from_slice(bytes),
            Self::Array(cells) => {
                for (slot, byte) in cells.iter().zip(bytes.iter()) {
                    *slot.borrow_mut() = Value::Int(i64::from(*byte));
                }
            }
        }
        Ok(())
    }

    pub(super) fn write_at(&self, offset: usize, bytes: &[u8]) -> EvalResult<()> {
        let end = offset.checked_add(bytes.len()).ok_or_else(|| {
            Halt::Trap("filesystem output byte range overflows host usize".to_owned())
        })?;
        self.require_capacity(end)?;
        match self {
            Self::Text { text, .. } => text.borrow_mut()[offset..end].copy_from_slice(bytes),
            Self::Array(cells) => {
                for (slot, byte) in cells[offset..end].iter().zip(bytes) {
                    *slot.borrow_mut() = Value::Int(i64::from(*byte));
                }
            }
        }
        Ok(())
    }
}

fn prepared_byte(cell: &Cell) -> EvalResult<u8> {
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

pub(super) struct PreparedMutableByteInput {
    #[allow(dead_code)]
    pub(super) output: PreparedByteOutput,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct PreparedI64Output {
    cell: Cell,
    pub(super) initial: i64,
}

impl PreparedI64Output {
    pub(super) fn write(&self, value: i64) -> EvalResult<()> {
        *self.cell.borrow_mut() = Value::Int(value);
        Ok(())
    }

    fn snapshot(&self) -> EvalResult<i64> {
        self.cell
            .borrow()
            .as_int()
            .ok_or_else(|| Halt::Trap("filesystem mutable scalar became non-integer".to_owned()))
    }
}

/// Every canonical authored operand is represented, including ABI-shape
/// operands a modeled provider does not otherwise need.
#[allow(dead_code)]
pub(super) enum PreparedFilesystemCall {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreparedFilesystemLogicalHandleInput {
    pub(super) operand_ordinal: u8,
    pub(super) kind: FilesystemLogicalHandleKind,
    pub(super) raw: i64,
    pub(super) null_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesystemLogicalHandleResultSuccess {
    NonNegative,
    NotMinusOne,
    Zero,
    NonZero,
}

impl FilesystemLogicalHandleResultSuccess {
    pub(super) const fn accepts(self, result: i64) -> bool {
        match self {
            Self::NonNegative => result >= 0,
            Self::NotMinusOne => result != -1,
            Self::Zero => result == 0,
            Self::NonZero => result != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparedFilesystemLogicalHandleOutput {
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
pub(super) enum FilesystemLogicalHandleRetirementSuccess {
    Zero,
    NonZero,
}

impl FilesystemLogicalHandleRetirementSuccess {
    pub(super) const fn accepts(self, result: i64) -> bool {
        match self {
            Self::Zero => result == 0,
            Self::NonZero => result != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PreparedFilesystemLogicalHandleRetirement {
    pub(super) operand_ordinal: u8,
    pub(super) success: FilesystemLogicalHandleRetirementSuccess,
}

pub(super) struct PreparedFilesystemLogicalHandlePlan {
    pub(super) inputs: Vec<PreparedFilesystemLogicalHandleInput>,
    pub(super) input_success: Option<FilesystemLogicalHandleResultSuccess>,
    pub(super) output: Option<PreparedFilesystemLogicalHandleOutput>,
    pub(super) retirement: Option<PreparedFilesystemLogicalHandleRetirement>,
}

fn observed_scalar(
    operand_ordinal: u8,
    value: FilesystemScalarOperandValue,
) -> FilesystemScalarOperand {
    FilesystemScalarOperand {
        operand_ordinal,
        value,
    }
}

fn observed_bytes(operand_ordinal: u8, value: &[u8]) -> FilesystemByteOperand {
    FilesystemByteOperand {
        operand_ordinal,
        bytes: value.to_vec(),
    }
}

pub(super) struct PreparedFilesystemMutableByteObservation {
    operand_ordinal: u8,
    output: PreparedByteOutput,
    pre_bytes: Vec<u8>,
}

pub(super) struct PreparedFilesystemMutableI64Observation {
    operand_ordinal: u8,
    output: PreparedI64Output,
    pre_value: i64,
}

pub(super) struct PreparedFilesystemMutableObservationPlan {
    byte_operands: Vec<PreparedFilesystemMutableByteObservation>,
    i64_operands: Vec<PreparedFilesystemMutableI64Observation>,
}

impl PreparedFilesystemMutableObservationPlan {
    pub(super) fn reserved_bytes(&self) -> Option<usize> {
        self.byte_operands
            .iter()
            .try_fold(0usize, |total, operand| {
                operand
                    .pre_bytes
                    .len()
                    .checked_mul(2)
                    .and_then(|bytes| total.checked_add(bytes))
            })
    }

    pub(super) fn initial_rows(
        &self,
    ) -> (
        Vec<FilesystemMutableByteOperand>,
        Vec<FilesystemMutableI64Operand>,
    ) {
        (
            self.byte_operands
                .iter()
                .map(|operand| FilesystemMutableByteOperand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_bytes: operand.pre_bytes.clone(),
                    post_bytes: operand.pre_bytes.clone(),
                })
                .collect(),
            self.i64_operands
                .iter()
                .map(|operand| FilesystemMutableI64Operand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_value: operand.pre_value,
                    post_value: operand.pre_value,
                })
                .collect(),
        )
    }

    pub(super) fn completed_rows(
        &self,
    ) -> EvalResult<(
        Vec<FilesystemMutableByteOperand>,
        Vec<FilesystemMutableI64Operand>,
    )> {
        let byte_operands = self
            .byte_operands
            .iter()
            .map(|operand| {
                Ok(FilesystemMutableByteOperand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_bytes: operand.pre_bytes.clone(),
                    post_bytes: operand.output.snapshot()?,
                })
            })
            .collect::<EvalResult<Vec<_>>>()?;
        let i64_operands = self
            .i64_operands
            .iter()
            .map(|operand| {
                Ok(FilesystemMutableI64Operand {
                    operand_ordinal: operand.operand_ordinal,
                    pre_value: operand.pre_value,
                    post_value: operand.output.snapshot()?,
                })
            })
            .collect::<EvalResult<Vec<_>>>()?;
        Ok((byte_operands, i64_operands))
    }
}

fn mutable_byte_observation(
    operand_ordinal: u8,
    output: &PreparedByteOutput,
) -> EvalResult<PreparedFilesystemMutableByteObservation> {
    Ok(PreparedFilesystemMutableByteObservation {
        operand_ordinal,
        output: output.clone(),
        pre_bytes: output.snapshot()?,
    })
}

fn mutable_i64_observation(
    operand_ordinal: u8,
    output: &PreparedI64Output,
) -> EvalResult<PreparedFilesystemMutableI64Observation> {
    Ok(PreparedFilesystemMutableI64Observation {
        operand_ordinal,
        output: output.clone(),
        pre_value: output.snapshot()?,
    })
}

impl PreparedFilesystemCall {
    /// Project the closed canonical call into descriptor/handle roles before a
    /// provider consumes it. Scalar values that merely share an integer ABI
    /// width (pointers, offsets, flags, ownership IDs) never enter this plan.
    pub(super) fn logical_handle_plan(&self) -> PreparedFilesystemLogicalHandlePlan {
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

    /// Project canonical non-handle scalars and immutable payload bytes from a
    /// fully prepared call. Path spellings and path-like byte aliases are
    /// deliberately excluded: scoped path evidence owns their portable rooted
    /// form and must never be bypassed by a raw absolute spelling.
    pub(super) fn operand_observation_plan(
        &self,
    ) -> (Vec<FilesystemScalarOperand>, Vec<FilesystemByteOperand>) {
        use FilesystemScalarOperandValue as Scalar;

        let mut scalars = Vec::new();
        let mut bytes = Vec::new();

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
            Self::CreateDir { mode, .. } | Self::CreateDirName { mode, .. } => {
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
            Self::Close { .. }
            | Self::Remove { .. }
            | Self::RemoveDir { .. }
            | Self::Rename { .. }
            | Self::HardLink { .. }
            | Self::Symlink { .. }
            | Self::Canonicalize { .. }
            | Self::FindFirst { .. }
            | Self::FindNext { .. }
            | Self::FindClose { .. }
            | Self::CloseHandle { .. }
            | Self::GetOsfHandle { .. }
            | Self::GetLastError
            | Self::RemoveName { .. }
            | Self::RemoveDirName { .. }
            | Self::ReadMetadata { .. }
            | Self::ReadFileMetadata { .. }
            | Self::ReadSymlinkMetadata { .. }
            | Self::SetFileTimes { .. }
            | Self::Sync { .. }
            | Self::SyncData { .. }
            | Self::Duplicate { .. }
            | Self::Errno => {}
        }
        (scalars, bytes)
    }

    /// Snapshot mutable carriers only after all authored arguments have been
    /// evaluated. A later argument may alias an earlier carrier, so capturing
    /// while the argument cursor advances would not describe provider-visible
    /// pre-state.
    pub(super) fn mutable_observation_plan(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn array_output(length: usize) -> PreparedByteOutput {
        PreparedByteOutput::Array(
            (0..length)
                .map(|_| Value::Int(0).cell())
                .collect::<Vec<_>>(),
        )
    }

    fn transfer_count() -> PreparedTransferCount {
        PreparedTransferCount { raw: 1, host: 1 }
    }

    fn mutable_byte_input(length: usize) -> PreparedMutableByteInput {
        PreparedMutableByteInput {
            output: array_output(length),
            bytes: vec![0; length],
        }
    }

    fn mutable_i64() -> PreparedI64Output {
        PreparedI64Output {
            cell: Value::Int(7).cell(),
            initial: 7,
        }
    }

    fn prepared_call_fixture(operation: FilesystemHostOperation) -> PreparedFilesystemCall {
        let path = || b"/root/file".to_vec();
        let name = || b"entry".to_vec();
        match operation {
            FilesystemHostOperation::Create => PreparedFilesystemCall::Create {
                path: path(),
                mode: -1,
            },
            FilesystemHostOperation::Open => PreparedFilesystemCall::Open {
                path: path(),
                flags: -1,
            },
            FilesystemHostOperation::OpenCreate => PreparedFilesystemCall::OpenCreate {
                path: path(),
                flags: -1,
                mode: -1,
            },
            FilesystemHostOperation::Read => PreparedFilesystemCall::Read {
                fd: 3,
                buffer: array_output(1),
                count: transfer_count(),
            },
            FilesystemHostOperation::Write => PreparedFilesystemCall::Write {
                fd: 3,
                bytes: b"payload".to_vec(),
            },
            FilesystemHostOperation::ReadAt => PreparedFilesystemCall::ReadAt {
                fd: 3,
                buffer: array_output(1),
                count: transfer_count(),
                offset: i64::MIN,
            },
            FilesystemHostOperation::WriteAt => PreparedFilesystemCall::WriteAt {
                fd: 3,
                bytes: b"payload".to_vec(),
                offset: i64::MIN,
            },
            FilesystemHostOperation::Close => PreparedFilesystemCall::Close { fd: 3 },
            FilesystemHostOperation::Remove => PreparedFilesystemCall::Remove { path: path() },
            FilesystemHostOperation::Seek => PreparedFilesystemCall::Seek {
                fd: 3,
                offset: i64::MIN,
                whence: -1,
            },
            FilesystemHostOperation::CreateDir => PreparedFilesystemCall::CreateDir {
                path: path(),
                mode: -1,
            },
            FilesystemHostOperation::RemoveDir => {
                PreparedFilesystemCall::RemoveDir { path: path() }
            }
            FilesystemHostOperation::CreateDirName => PreparedFilesystemCall::CreateDirName {
                name: path(),
                mode: -1,
            },
            FilesystemHostOperation::OpenAt => PreparedFilesystemCall::OpenAt {
                dirfd: 3,
                name: name(),
                flags: -1,
            },
            FilesystemHostOperation::UnlinkAt => PreparedFilesystemCall::UnlinkAt {
                dirfd: 3,
                name: name(),
                flags: -1,
            },
            FilesystemHostOperation::SetPermissions => PreparedFilesystemCall::SetPermissions {
                path: path(),
                mode: u32::MAX,
            },
            FilesystemHostOperation::SetFilePermissions => {
                PreparedFilesystemCall::SetFilePermissions {
                    fd: 3,
                    mode: u32::MAX,
                }
            }
            FilesystemHostOperation::Rename => PreparedFilesystemCall::Rename {
                from: path(),
                to: path(),
            },
            FilesystemHostOperation::HardLink => PreparedFilesystemCall::HardLink {
                original: path(),
                link: path(),
            },
            FilesystemHostOperation::Symlink => PreparedFilesystemCall::Symlink {
                target: path(),
                link: path(),
            },
            FilesystemHostOperation::ReadLink => PreparedFilesystemCall::ReadLink {
                path: path(),
                buffer: array_output(1),
                count: transfer_count(),
            },
            FilesystemHostOperation::Canonicalize => PreparedFilesystemCall::Canonicalize {
                path: path(),
                buffer: array_output(PATH_MAX_OUTPUT_BYTES),
            },
            FilesystemHostOperation::ReadDir => PreparedFilesystemCall::ReadDir {
                fd: 3,
                buffer: array_output(1),
                count: transfer_count(),
                position: mutable_i64(),
            },
            FilesystemHostOperation::FindFirst => PreparedFilesystemCall::FindFirst {
                pattern: path(),
                data: array_output(FIND_DATA_OUTPUT_BYTES),
            },
            FilesystemHostOperation::FindNext => PreparedFilesystemCall::FindNext {
                handle: 11,
                data: array_output(FIND_DATA_OUTPUT_BYTES),
            },
            FilesystemHostOperation::FindClose => PreparedFilesystemCall::FindClose { handle: 11 },
            FilesystemHostOperation::CreateHardLink => PreparedFilesystemCall::CreateHardLink {
                link: path(),
                existing: path(),
                security_attributes: i64::MIN,
            },
            FilesystemHostOperation::OpenPathHandle => PreparedFilesystemCall::OpenPathHandle {
                path: path(),
                desired_access: u32::MAX,
                share_mode: u32::MAX,
                security_attributes: i64::MIN,
                creation_disposition: u32::MAX,
                flags_and_attributes: u32::MAX,
                template_file: 0,
            },
            FilesystemHostOperation::CloseHandle => {
                PreparedFilesystemCall::CloseHandle { handle: 3 }
            }
            FilesystemHostOperation::GetOsfHandle => PreparedFilesystemCall::GetOsfHandle { fd: 3 },
            FilesystemHostOperation::FinalPathNameByHandle => {
                PreparedFilesystemCall::FinalPathNameByHandle {
                    handle: 3,
                    buffer: array_output(1),
                    capacity: transfer_count(),
                    flags: u32::MAX,
                }
            }
            FilesystemHostOperation::SetFileTime => PreparedFilesystemCall::SetFileTime {
                handle: 3,
                creation: i64::MIN,
                last_access: vec![1; FILETIME_BYTES + 1],
                last_write: vec![2; FILETIME_BYTES + 1],
            },
            FilesystemHostOperation::LockFileEx => PreparedFilesystemCall::LockFileEx {
                handle: 3,
                flags: u32::MAX,
                reserved: u32::MAX,
                length_low: u32::MAX,
                length_high: u32::MAX,
                overlapped: mutable_byte_input(OVERLAPPED_BYTES),
            },
            FilesystemHostOperation::UnlockFile => PreparedFilesystemCall::UnlockFile {
                handle: 3,
                offset_low: u32::MAX,
                offset_high: u32::MAX,
                length_low: u32::MAX,
                length_high: u32::MAX,
            },
            FilesystemHostOperation::GetLastError => PreparedFilesystemCall::GetLastError,
            FilesystemHostOperation::RemoveName => {
                PreparedFilesystemCall::RemoveName { path: path() }
            }
            FilesystemHostOperation::RemoveDirName => {
                PreparedFilesystemCall::RemoveDirName { path: path() }
            }
            FilesystemHostOperation::ReadMetadata => PreparedFilesystemCall::ReadMetadata {
                path: path(),
                buffer: array_output(STAT_OUTPUT_BYTES),
            },
            FilesystemHostOperation::ReadFileMetadata => PreparedFilesystemCall::ReadFileMetadata {
                fd: 3,
                buffer: array_output(STAT_OUTPUT_BYTES),
            },
            FilesystemHostOperation::ReadSymlinkMetadata => {
                PreparedFilesystemCall::ReadSymlinkMetadata {
                    path: path(),
                    buffer: array_output(STAT_OUTPUT_BYTES),
                }
            }
            FilesystemHostOperation::SetLen => PreparedFilesystemCall::SetLen {
                fd: 3,
                length: i64::MIN,
            },
            FilesystemHostOperation::SetFileTimes => PreparedFilesystemCall::SetFileTimes {
                fd: 3,
                times: mutable_byte_input(TIMESPEC_PAIR_BYTES),
            },
            FilesystemHostOperation::Sync => PreparedFilesystemCall::Sync { fd: 3 },
            FilesystemHostOperation::SyncData => PreparedFilesystemCall::SyncData { fd: 3 },
            FilesystemHostOperation::Duplicate => PreparedFilesystemCall::Duplicate { fd: 3 },
            FilesystemHostOperation::LockFile => PreparedFilesystemCall::LockFile {
                fd: 3,
                operation: -1,
            },
            FilesystemHostOperation::ChangeOwner => PreparedFilesystemCall::ChangeOwner {
                path: path(),
                uid: -1,
                gid: -1,
            },
            FilesystemHostOperation::ChangeOwnerNoFollow => {
                PreparedFilesystemCall::ChangeOwnerNoFollow {
                    path: path(),
                    uid: -1,
                    gid: -1,
                }
            }
            FilesystemHostOperation::ChangeFileOwner => PreparedFilesystemCall::ChangeFileOwner {
                fd: 3,
                uid: -1,
                gid: -1,
            },
            FilesystemHostOperation::Errno => PreparedFilesystemCall::Errno,
        }
    }

    fn expected_immutable_byte_ordinals(operation: FilesystemHostOperation) -> &'static [u8] {
        match operation {
            FilesystemHostOperation::Write | FilesystemHostOperation::WriteAt => &[1],
            FilesystemHostOperation::OpenAt | FilesystemHostOperation::UnlinkAt => &[1],
            FilesystemHostOperation::SetFileTime => &[2, 3],
            FilesystemHostOperation::Create
            | FilesystemHostOperation::Open
            | FilesystemHostOperation::OpenCreate
            | FilesystemHostOperation::Read
            | FilesystemHostOperation::ReadAt
            | FilesystemHostOperation::Close
            | FilesystemHostOperation::Remove
            | FilesystemHostOperation::Seek
            | FilesystemHostOperation::CreateDir
            | FilesystemHostOperation::RemoveDir
            | FilesystemHostOperation::CreateDirName
            | FilesystemHostOperation::SetPermissions
            | FilesystemHostOperation::SetFilePermissions
            | FilesystemHostOperation::Rename
            | FilesystemHostOperation::HardLink
            | FilesystemHostOperation::Symlink
            | FilesystemHostOperation::ReadLink
            | FilesystemHostOperation::Canonicalize
            | FilesystemHostOperation::ReadDir
            | FilesystemHostOperation::FindFirst
            | FilesystemHostOperation::FindNext
            | FilesystemHostOperation::FindClose
            | FilesystemHostOperation::CreateHardLink
            | FilesystemHostOperation::OpenPathHandle
            | FilesystemHostOperation::CloseHandle
            | FilesystemHostOperation::GetOsfHandle
            | FilesystemHostOperation::FinalPathNameByHandle
            | FilesystemHostOperation::LockFileEx
            | FilesystemHostOperation::UnlockFile
            | FilesystemHostOperation::GetLastError
            | FilesystemHostOperation::RemoveName
            | FilesystemHostOperation::RemoveDirName
            | FilesystemHostOperation::ReadMetadata
            | FilesystemHostOperation::ReadFileMetadata
            | FilesystemHostOperation::ReadSymlinkMetadata
            | FilesystemHostOperation::SetLen
            | FilesystemHostOperation::SetFileTimes
            | FilesystemHostOperation::Sync
            | FilesystemHostOperation::SyncData
            | FilesystemHostOperation::Duplicate
            | FilesystemHostOperation::LockFile
            | FilesystemHostOperation::ChangeOwner
            | FilesystemHostOperation::ChangeOwnerNoFollow
            | FilesystemHostOperation::ChangeFileOwner
            | FilesystemHostOperation::Errno => &[],
        }
    }

    #[test]
    fn transfer_counts_are_bounded_without_losing_the_authored_value() {
        assert_eq!(
            checked_filesystem_transfer_count(-1),
            Err(FilesystemTransferCountError::NegativeOrUnrepresentable)
        );
        assert_eq!(
            checked_filesystem_transfer_count(MAX_FILESYSTEM_TRANSFER_BYTES as i64 + 1),
            Err(FilesystemTransferCountError::ExceedsEvaluatorLimit)
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
    fn prepared_output_rejects_truncation_and_keeps_the_resolved_cells() {
        let output = array_output(2);
        let cells = match &output {
            PreparedByteOutput::Array(cells) => cells.clone(),
            PreparedByteOutput::Text { .. } => unreachable!(),
        };
        assert!(output.write(&[1, 2, 3]).is_err());
        assert!(output.write(&[7, 8]).is_ok());
        assert_eq!(cells[0].borrow().as_int(), Some(7));
        assert_eq!(cells[1].borrow().as_int(), Some(8));
    }

    #[test]
    fn prepared_text_output_preserves_its_original_capacity_across_short_writes() {
        let text = std::rc::Rc::new(std::cell::RefCell::new(vec![0; 4]));
        let output = PreparedByteOutput::Text {
            text: std::rc::Rc::clone(&text),
            capacity: 4,
        };
        assert!(output.write(&[1]).is_ok());
        assert!(output.write(&[2, 3, 4, 5]).is_ok());
        assert_eq!(&*text.borrow(), &[2, 3, 4, 5]);
    }

    #[test]
    fn synthetic_handles_never_alias_after_narrowing() {
        assert_eq!(synthetic_handle_fd(3), Some(3));
        assert_eq!(synthetic_handle_fd(0x1_0000_0003), None);
        assert_eq!(synthetic_handle_fd(i64::from(i32::MIN) - 1), None);
    }

    #[test]
    fn at_family_names_are_exact_portable_relative_components() {
        let Ok(accepted) = checked_relative_component(b"entry.bin".to_vec()) else {
            panic!("one ordinary component must be accepted")
        };
        assert_eq!(accepted, b"entry.bin");
        for rejected in [
            b"".as_slice(),
            b".".as_slice(),
            b"..".as_slice(),
            b"nested/entry".as_slice(),
            b"nested\\entry".as_slice(),
            b"nul\0entry".as_slice(),
        ] {
            assert!(
                checked_relative_component(rejected.to_vec()).is_err(),
                "unexpected accepted relative component: {rejected:?}"
            );
        }
    }

    #[test]
    fn every_canonical_operation_rejects_wrong_arity_before_cursor_creation() {
        for operation in FilesystemHostOperation::ALL {
            let expected = operation.operand_kinds().len();
            assert!(check_filesystem_arity(operation, expected).is_ok());
            assert!(check_filesystem_arity(operation, expected + 1).is_err());
            if expected > 0 {
                assert!(check_filesystem_arity(operation, expected - 1).is_err());
            }
        }
    }

    #[test]
    fn prepared_byte_snapshots_reject_wrong_element_kinds_and_ranges() {
        let wrong_kind = PreparedByteOutput::Array(vec![Value::Bool(true).cell()]);
        assert!(wrong_kind.snapshot().is_err());
        let wrong_range = PreparedByteOutput::Array(vec![Value::Int(256).cell()]);
        assert!(wrong_range.snapshot().is_err());
    }

    #[test]
    fn provider_boundaries_only_accept_prepared_calls() {
        let virtual_source = include_str!("filesystem.rs");
        let real_source = include_str!("../evaluator_real_fs.rs");
        assert!(
            virtual_source
                .contains("fn serve_filesystem_call(&mut self, call: PreparedFilesystemCall)")
        );
        assert!(real_source.contains("call: PreparedFilesystemCall,"));
        assert!(!real_source.contains("ExpressionHandle"));
        assert!(!real_source.contains("frame: &Frame"));
        assert!(!virtual_source.contains("handle as i32"));
        assert!(!real_source.contains("handle as i32"));
    }

    #[test]
    fn logical_handle_plan_distinguishes_descriptor_native_find_and_pointer_scalars() {
        let descriptor_open = PreparedFilesystemCall::Create {
            path: b"file".to_vec(),
            mode: 0,
        }
        .logical_handle_plan();
        assert!(descriptor_open.inputs.is_empty());
        assert!(matches!(
            descriptor_open.output,
            Some(PreparedFilesystemLogicalHandleOutput::Created {
                kind: FilesystemLogicalHandleKind::Descriptor,
                ..
            })
        ));

        let borrowed = PreparedFilesystemCall::GetOsfHandle { fd: 7 }.logical_handle_plan();
        assert_eq!(
            borrowed.inputs,
            vec![PreparedFilesystemLogicalHandleInput {
                operand_ordinal: 0,
                kind: FilesystemLogicalHandleKind::Descriptor,
                raw: 7,
                null_allowed: false,
            }]
        );
        assert!(matches!(
            borrowed.output,
            Some(PreparedFilesystemLogicalHandleOutput::Borrowed {
                source_operand_ordinal: 0,
                ..
            })
        ));

        let native_open = PreparedFilesystemCall::OpenPathHandle {
            path: b"file".to_vec(),
            desired_access: 0,
            share_mode: 0,
            security_attributes: 123,
            creation_disposition: 0,
            flags_and_attributes: 0,
            template_file: 0,
        }
        .logical_handle_plan();
        assert_eq!(
            native_open.inputs,
            vec![PreparedFilesystemLogicalHandleInput {
                operand_ordinal: 6,
                kind: FilesystemLogicalHandleKind::Native,
                raw: 0,
                null_allowed: true,
            }],
            "security_attributes is pointer-shaped but only template_file is a handle"
        );

        let find = PreparedFilesystemCall::FindFirst {
            pattern: b"dir/*".to_vec(),
            data: array_output(FIND_DATA_OUTPUT_BYTES),
        }
        .logical_handle_plan();
        assert!(matches!(
            find.output,
            Some(PreparedFilesystemLogicalHandleOutput::Created {
                kind: FilesystemLogicalHandleKind::Find,
                ..
            })
        ));

        let hard_link = PreparedFilesystemCall::CreateHardLink {
            link: b"link".to_vec(),
            existing: b"file".to_vec(),
            security_attributes: 123,
        }
        .logical_handle_plan();
        assert!(hard_link.inputs.is_empty());
        assert!(hard_link.output.is_none());
    }

    #[test]
    fn operand_observation_plan_preserves_width_payload_and_safe_components() {
        let open_at = PreparedFilesystemCall::OpenAt {
            dirfd: 7,
            name: b"entry.bin".to_vec(),
            flags: -1,
        };
        let (scalars, bytes) = open_at.operand_observation_plan();
        assert_eq!(scalars.len(), 1);
        assert_eq!(scalars[0].operand_ordinal(), 2);
        assert_eq!(scalars[0].value(), FilesystemScalarOperandValue::I32(-1));
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes[0].operand_ordinal(), 1);
        assert_eq!(bytes[0].bytes(), b"entry.bin");

        let write = PreparedFilesystemCall::Write {
            fd: 3,
            bytes: b"a\0b".to_vec(),
        };
        let (scalars, bytes) = write.operand_observation_plan();
        assert!(scalars.is_empty(), "raw descriptor tokens are not scalars");
        assert_eq!(bytes[0].bytes(), b"a\0b");

        let set_time = PreparedFilesystemCall::SetFileTime {
            handle: 3,
            creation: i64::MIN,
            last_access: (0u8..12).collect(),
            last_write: (20u8..32).collect(),
        };
        let (scalars, bytes) = set_time.operand_observation_plan();
        assert_eq!(
            scalars[0].value(),
            FilesystemScalarOperandValue::I64(i64::MIN)
        );
        assert_eq!(bytes[0].bytes(), &(0u8..12).collect::<Vec<_>>());
        assert_eq!(bytes[1].bytes(), &(20u8..32).collect::<Vec<_>>());

        let native_open = PreparedFilesystemCall::OpenPathHandle {
            path: b"file".to_vec(),
            desired_access: u32::MAX,
            share_mode: 2,
            security_attributes: 123,
            creation_disposition: 4,
            flags_and_attributes: 5,
            template_file: 99,
        };
        let (scalars, bytes) = native_open.operand_observation_plan();
        assert_eq!(
            scalars
                .iter()
                .map(|operand| operand.operand_ordinal())
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5],
            "path and logical-handle ordinals stay outside scalar evidence"
        );
        assert_eq!(
            scalars[0].value(),
            FilesystemScalarOperandValue::U32(u32::MAX)
        );
        assert!(bytes.is_empty());
    }

    #[test]
    fn every_canonical_operation_has_exact_scalar_and_immutable_byte_roles() {
        use super::super::filesystem_host_operation::FilesystemHostOperandKind as Kind;

        for operation in FilesystemHostOperation::ALL {
            let call = prepared_call_fixture(operation);
            let logical_plan = call.logical_handle_plan();
            let mutable_plan = call
                .mutable_observation_plan()
                .unwrap_or_else(|_| panic!("mutable observation fixture must be representable"));
            let logical_ordinals = logical_plan
                .inputs
                .iter()
                .map(|input| input.operand_ordinal)
                .collect::<std::collections::BTreeSet<_>>();
            let (scalars, bytes) = call.operand_observation_plan();
            let actual_scalars = scalars
                .iter()
                .map(|operand| {
                    let kind = match operand.value() {
                        FilesystemScalarOperandValue::I32(_) => Kind::I32,
                        FilesystemScalarOperandValue::U32(_) => Kind::U32,
                        FilesystemScalarOperandValue::I64(_) => Kind::I64,
                        FilesystemScalarOperandValue::U64(_) => Kind::U64,
                    };
                    (operand.operand_ordinal(), kind)
                })
                .collect::<Vec<_>>();
            let expected_scalars = operation
                .operand_kinds()
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(ordinal, kind)| {
                    let ordinal = u8::try_from(ordinal).unwrap();
                    matches!(kind, Kind::I32 | Kind::U32 | Kind::I64 | Kind::U64)
                        .then_some((ordinal, kind))
                        .filter(|(ordinal, _)| !logical_ordinals.contains(ordinal))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                actual_scalars, expected_scalars,
                "scalar evidence role drift for `{operation}`"
            );

            let actual_bytes = bytes
                .iter()
                .map(|operand| operand.operand_ordinal())
                .collect::<Vec<_>>();
            assert_eq!(
                actual_bytes,
                expected_immutable_byte_ordinals(operation),
                "immutable byte evidence role drift for `{operation}`"
            );
            let expected_mutable_bytes = operation
                .operand_kinds()
                .iter()
                .enumerate()
                .filter_map(|(ordinal, kind)| {
                    (*kind == Kind::MutableBytes).then_some(u8::try_from(ordinal).unwrap())
                })
                .collect::<Vec<_>>();
            assert_eq!(
                mutable_plan
                    .byte_operands
                    .iter()
                    .map(|operand| operand.operand_ordinal)
                    .collect::<Vec<_>>(),
                expected_mutable_bytes,
                "mutable byte evidence role drift for `{operation}`"
            );
            let expected_mutable_i64 = operation
                .operand_kinds()
                .iter()
                .enumerate()
                .filter_map(|(ordinal, kind)| {
                    (*kind == Kind::MutableI64).then_some(u8::try_from(ordinal).unwrap())
                })
                .collect::<Vec<_>>();
            assert_eq!(
                mutable_plan
                    .i64_operands
                    .iter()
                    .map(|operand| operand.operand_ordinal)
                    .collect::<Vec<_>>(),
                expected_mutable_i64,
                "mutable i64 evidence role drift for `{operation}`"
            );

            let mut observed_ordinals = logical_ordinals;
            for ordinal in actual_scalars
                .iter()
                .map(|(ordinal, _)| *ordinal)
                .chain(actual_bytes.iter().copied())
            {
                assert!(
                    observed_ordinals.insert(ordinal),
                    "operand `{ordinal}` of `{operation}` entered two evidence roles"
                );
            }
        }
    }

    #[test]
    fn mutable_observation_plan_retains_complete_pre_and_post_carriers() {
        let buffer = array_output(3);
        buffer
            .write(&[9, 8, 7])
            .unwrap_or_else(|_| panic!("fixture write must fit"));
        let call = PreparedFilesystemCall::Read {
            fd: 3,
            buffer: buffer.clone(),
            count: PreparedTransferCount { raw: 1, host: 1 },
        };
        let plan = call
            .mutable_observation_plan()
            .unwrap_or_else(|_| panic!("mutable observation fixture must be representable"));
        assert_eq!(plan.reserved_bytes(), Some(6));
        let (initial_bytes, initial_i64) = plan.initial_rows();
        assert!(initial_i64.is_empty());
        assert_eq!(initial_bytes[0].pre_bytes(), &[9, 8, 7]);
        assert_eq!(initial_bytes[0].post_bytes(), &[9, 8, 7]);

        buffer
            .write(&[1])
            .unwrap_or_else(|_| panic!("fixture write must fit"));
        let (completed_bytes, completed_i64) = plan
            .completed_rows()
            .unwrap_or_else(|_| panic!("completed fixture must be representable"));
        assert!(completed_i64.is_empty());
        assert_eq!(completed_bytes[0].pre_bytes(), &[9, 8, 7]);
        assert_eq!(
            completed_bytes[0].post_bytes(),
            &[1, 8, 7],
            "unchanged mutable tail remains explicit"
        );

        let position = mutable_i64();
        let call = PreparedFilesystemCall::ReadDir {
            fd: 3,
            buffer: array_output(1),
            count: transfer_count(),
            position: position.clone(),
        };
        let plan = call
            .mutable_observation_plan()
            .unwrap_or_else(|_| panic!("mutable observation fixture must be representable"));
        position
            .write(12)
            .unwrap_or_else(|_| panic!("fixture cursor write must fit"));
        let (_, completed_i64) = plan
            .completed_rows()
            .unwrap_or_else(|_| panic!("completed fixture must be representable"));
        assert_eq!(completed_i64[0].pre_value(), 7);
        assert_eq!(completed_i64[0].post_value(), 12);
    }

    #[test]
    fn operation_attempt_encloses_canonical_preparation() {
        let source = include_str!("filesystem.rs");
        let push = source.find("push(attempt_index)").expect("attempt push");
        let prepare = source
            .find(".prepare_filesystem_call(operation, arguments, frame)")
            .expect("canonical preparation");
        let pop = source[prepare..]
            .find(".pop()")
            .map(|offset| prepare + offset)
            .expect("attempt pop");
        assert!(push < prepare && prepare < pop);
    }
}

struct FilesystemArgumentCursor<'evaluation, 'program, 'arguments, 'frame> {
    evaluator: &'evaluation mut Evaluator<'program>,
    arguments: std::slice::Iter<'arguments, ExpressionHandle>,
    frame: &'frame Frame,
    consumed: usize,
}

impl<'evaluation, 'program, 'arguments, 'frame>
    FilesystemArgumentCursor<'evaluation, 'program, 'arguments, 'frame>
{
    fn new(
        evaluator: &'evaluation mut Evaluator<'program>,
        arguments: &'arguments [ExpressionHandle],
        frame: &'frame Frame,
    ) -> Self {
        Self {
            evaluator,
            arguments: arguments.iter(),
            frame,
            consumed: 0,
        }
    }

    fn next(&mut self) -> EvalResult<ExpressionHandle> {
        let handle = self.arguments.next().copied().ok_or_else(|| {
            Halt::Trap("canonical filesystem call is missing an authored operand".to_owned())
        })?;
        self.consumed += 1;
        Ok(handle)
    }

    fn value(&mut self) -> EvalResult<Value> {
        let handle = self.next()?;
        self.evaluator.eval_expression(handle, self.frame)
    }

    fn i64(&mut self) -> EvalResult<i64> {
        match self.value()? {
            Value::Int(value) => Ok(value),
            _ => trap("canonical filesystem scalar operand is not an integer"),
        }
    }

    fn i32(&mut self) -> EvalResult<i32> {
        let raw = self.i64()?;
        i32::try_from(raw).map_err(|_| {
            Halt::Trap(format!(
                "canonical filesystem i32 operand `{raw}` is out of range"
            ))
        })
    }

    fn u32(&mut self) -> EvalResult<u32> {
        let raw = self.i64()?;
        u32::try_from(raw).map_err(|_| {
            Halt::Trap(format!(
                "canonical filesystem u32 operand `{raw}` is out of range"
            ))
        })
    }

    fn fd(&mut self) -> EvalResult<i32> {
        let value = self.value()?;
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
        i32::try_from(raw).map_err(|_| Halt::Trap(format!("filesystem fd `{raw}` is out of range")))
    }

    fn count(&mut self) -> EvalResult<PreparedTransferCount> {
        let raw = self.i64()?;
        checked_filesystem_transfer_count(raw).map_err(|error| match error {
            FilesystemTransferCountError::NegativeOrUnrepresentable => Halt::Trap(
                "filesystem transfer count is negative or not host-representable".to_owned(),
            ),
            FilesystemTransferCountError::ExceedsEvaluatorLimit => Halt::Trap(format!(
                "filesystem transfer count exceeds evaluator limit of {MAX_FILESYSTEM_TRANSFER_BYTES} bytes"
            )),
        })
    }

    fn bytes(&mut self) -> EvalResult<Vec<u8>> {
        let value = self.value()?;
        if let Some((_, relative)) = rooted_build_path_parts(&value)? {
            check_byte_len(relative.len())?;
            return Ok(relative);
        }
        match value {
            Value::Str(text) => {
                let text = text.borrow();
                check_byte_len(text.len())?;
                Ok(text.clone())
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
        }
    }

    fn path(&mut self) -> EvalResult<Vec<u8>> {
        let value = self.value()?;
        let Some((root, relative)) = rooted_build_path_parts(&value)? else {
            if self.evaluator.rooted_build_paths_required
                && self
                    .evaluator
                    .real_fs
                    .as_ref()
                    .is_some_and(real_fs::RealFs::is_scoped)
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
                    Ok(text.clone())
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
        let filesystem = self.evaluator.real_fs.as_ref().ok_or_else(|| {
            Halt::Trap("rooted build path requires a scoped real filesystem".to_owned())
        })?;
        filesystem
            .rooted_path_bytes(root, &relative)
            .ok_or_else(|| {
                Halt::Trap("rooted build path names no compiler-supplied grant root".to_owned())
            })
    }

    fn mutable_bytes(&mut self) -> EvalResult<PreparedByteOutput> {
        let handle = self.next()?;
        let cell = self.evaluator.resolve_place(handle, self.frame)?;
        let cell = self.evaluator.deref_cell(cell);
        match &*cell.borrow() {
            Value::Str(text) => {
                let capacity = text.borrow().len();
                check_byte_len(capacity)?;
                Ok(PreparedByteOutput::Text {
                    text: std::rc::Rc::clone(text),
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
    }

    fn mutable_i64(&mut self) -> EvalResult<PreparedI64Output> {
        let handle = self.next()?;
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
        Ok(PreparedMutableByteInput { output, bytes })
    }

    fn finish(self) -> EvalResult<()> {
        if self.arguments.len() == 0 {
            Ok(())
        } else {
            trap("canonical filesystem call has unconsumed authored operands")
        }
    }
}

fn rooted_build_path_parts(
    value: &Value,
) -> EvalResult<Option<(FilesystemGrantRootIdentity, Vec<u8>)>> {
    let Value::Struct {
        type_name, fields, ..
    } = value
    else {
        return Ok(None);
    };
    if type_name != ROOTED_BUILD_PATH_TYPE {
        return Ok(None);
    }
    let root = fields
        .get("root")
        .and_then(|root| root.borrow().as_int())
        .and_then(|root| u32::try_from(root).ok())
        .and_then(FilesystemGrantRootIdentity::new)
        .ok_or_else(|| Halt::Trap("rooted build path has no valid root identity".to_owned()))?;
    let relative = fields
        .get("relative")
        .and_then(|relative| match &*relative.borrow() {
            Value::Str(bytes) => Some(bytes.borrow().clone()),
            _ => None,
        })
        .ok_or_else(|| Halt::Trap("rooted build path has no relative bytes".to_owned()))?;
    Ok(Some((root, relative)))
}

impl<'program> Evaluator<'program> {
    pub(super) fn prepare_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<PreparedFilesystemCall> {
        check_filesystem_arity(operation, arguments.len())?;
        if self.rooted_build_paths_required
            && matches!(
                operation,
                FilesystemHostOperation::Canonicalize
                    | FilesystemHostOperation::FinalPathNameByHandle
            )
        {
            return Err(Halt::Trap(format!(
                "package build filesystem operation `{}` would expose a host-absolute path",
                operation.canonical_name()
            )));
        }
        let mut a = FilesystemArgumentCursor::new(self, arguments, frame);
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
                let fd = a.fd()?;
                let buffer = a.mutable_bytes()?;
                let count = a.count()?;
                buffer.require_capacity(count.host)?;
                PreparedFilesystemCall::Read { fd, buffer, count }
            }
            FilesystemHostOperation::Write => PreparedFilesystemCall::Write {
                fd: a.fd()?,
                bytes: a.bytes()?,
            },
            FilesystemHostOperation::ReadAt => {
                let fd = a.fd()?;
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
                fd: a.fd()?,
                bytes: a.bytes()?,
                offset: a.i64()?,
            },
            FilesystemHostOperation::Close => PreparedFilesystemCall::Close { fd: a.fd()? },
            FilesystemHostOperation::Remove => PreparedFilesystemCall::Remove { path: a.path()? },
            FilesystemHostOperation::Seek => PreparedFilesystemCall::Seek {
                fd: a.fd()?,
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
                name: a.bytes()?,
                mode: a.i32()?,
            },
            FilesystemHostOperation::OpenAt => {
                let dirfd = a.fd()?;
                let name = checked_relative_component(a.bytes()?)?;
                let flags = a.i32()?;
                PreparedFilesystemCall::OpenAt { dirfd, name, flags }
            }
            FilesystemHostOperation::UnlinkAt => {
                let dirfd = a.fd()?;
                let name = checked_relative_component(a.bytes()?)?;
                let flags = a.i32()?;
                PreparedFilesystemCall::UnlinkAt { dirfd, name, flags }
            }
            FilesystemHostOperation::SetPermissions => PreparedFilesystemCall::SetPermissions {
                path: a.path()?,
                mode: a.u32()?,
            },
            FilesystemHostOperation::SetFilePermissions => {
                PreparedFilesystemCall::SetFilePermissions {
                    fd: a.fd()?,
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
                target: a.bytes()?,
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
                let fd = a.fd()?;
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
                let pattern = a.bytes()?;
                let data = a.mutable_bytes()?;
                data.require_capacity(FIND_DATA_OUTPUT_BYTES)?;
                PreparedFilesystemCall::FindFirst { pattern, data }
            }
            FilesystemHostOperation::FindNext => {
                let handle = a.i64()?;
                let data = a.mutable_bytes()?;
                data.require_capacity(FIND_DATA_OUTPUT_BYTES)?;
                PreparedFilesystemCall::FindNext { handle, data }
            }
            FilesystemHostOperation::FindClose => {
                PreparedFilesystemCall::FindClose { handle: a.i64()? }
            }
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
                template_file: a.i64()?,
            },
            FilesystemHostOperation::CloseHandle => {
                PreparedFilesystemCall::CloseHandle { handle: a.i64()? }
            }
            FilesystemHostOperation::GetOsfHandle => {
                PreparedFilesystemCall::GetOsfHandle { fd: a.fd()? }
            }
            FilesystemHostOperation::FinalPathNameByHandle => {
                let handle = a.i64()?;
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
                let handle = a.i64()?;
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
                handle: a.i64()?,
                flags: a.u32()?,
                reserved: a.u32()?,
                length_low: a.u32()?,
                length_high: a.u32()?,
                overlapped: a.mutable_byte_input(OVERLAPPED_BYTES)?,
            },
            FilesystemHostOperation::UnlockFile => PreparedFilesystemCall::UnlockFile {
                handle: a.i64()?,
                offset_low: a.u32()?,
                offset_high: a.u32()?,
                length_low: a.u32()?,
                length_high: a.u32()?,
            },
            FilesystemHostOperation::GetLastError => PreparedFilesystemCall::GetLastError,
            FilesystemHostOperation::RemoveName => {
                PreparedFilesystemCall::RemoveName { path: a.bytes()? }
            }
            FilesystemHostOperation::RemoveDirName => {
                PreparedFilesystemCall::RemoveDirName { path: a.bytes()? }
            }
            FilesystemHostOperation::ReadMetadata => {
                let path = a.path()?;
                let buffer = a.mutable_bytes()?;
                buffer.require_capacity(STAT_OUTPUT_BYTES)?;
                PreparedFilesystemCall::ReadMetadata { path, buffer }
            }
            FilesystemHostOperation::ReadFileMetadata => {
                let fd = a.fd()?;
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
                fd: a.fd()?,
                length: a.i64()?,
            },
            FilesystemHostOperation::SetFileTimes => PreparedFilesystemCall::SetFileTimes {
                fd: a.fd()?,
                times: a.mutable_byte_input(TIMESPEC_PAIR_BYTES)?,
            },
            FilesystemHostOperation::Sync => PreparedFilesystemCall::Sync { fd: a.fd()? },
            FilesystemHostOperation::SyncData => PreparedFilesystemCall::SyncData { fd: a.fd()? },
            FilesystemHostOperation::Duplicate => PreparedFilesystemCall::Duplicate { fd: a.fd()? },
            FilesystemHostOperation::LockFile => PreparedFilesystemCall::LockFile {
                fd: a.fd()?,
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
                fd: a.fd()?,
                uid: a.i32()?,
                gid: a.i32()?,
            },
            FilesystemHostOperation::Errno => PreparedFilesystemCall::Errno,
        };
        a.finish()?;
        Ok(call)
    }
}
