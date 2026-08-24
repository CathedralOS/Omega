use super::*;

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

pub(super) struct PreparedI64Output {
    cell: Cell,
    pub(super) initial: i64,
}

impl PreparedI64Output {
    pub(super) fn write(&self, value: i64) -> EvalResult<()> {
        *self.cell.borrow_mut() = Value::Int(value);
        Ok(())
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
        match self.value()? {
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

impl<'program> Evaluator<'program> {
    pub(super) fn prepare_filesystem_call(
        &mut self,
        operation: FilesystemHostOperation,
        arguments: &[ExpressionHandle],
        frame: &Frame,
    ) -> EvalResult<PreparedFilesystemCall> {
        check_filesystem_arity(operation, arguments.len())?;
        let mut a = FilesystemArgumentCursor::new(self, arguments, frame);
        let call = match operation {
            FilesystemHostOperation::Create => PreparedFilesystemCall::Create {
                path: a.bytes()?,
                mode: a.i32()?,
            },
            FilesystemHostOperation::Open => PreparedFilesystemCall::Open {
                path: a.bytes()?,
                flags: a.i32()?,
            },
            FilesystemHostOperation::OpenCreate => PreparedFilesystemCall::OpenCreate {
                path: a.bytes()?,
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
            FilesystemHostOperation::Remove => PreparedFilesystemCall::Remove { path: a.bytes()? },
            FilesystemHostOperation::Seek => PreparedFilesystemCall::Seek {
                fd: a.fd()?,
                offset: a.i64()?,
                whence: a.i32()?,
            },
            FilesystemHostOperation::CreateDir => PreparedFilesystemCall::CreateDir {
                path: a.bytes()?,
                mode: a.i32()?,
            },
            FilesystemHostOperation::RemoveDir => {
                PreparedFilesystemCall::RemoveDir { path: a.bytes()? }
            }
            FilesystemHostOperation::CreateDirName => PreparedFilesystemCall::CreateDirName {
                name: a.bytes()?,
                mode: a.i32()?,
            },
            FilesystemHostOperation::OpenAt => PreparedFilesystemCall::OpenAt {
                dirfd: a.fd()?,
                name: a.bytes()?,
                flags: a.i32()?,
            },
            FilesystemHostOperation::UnlinkAt => PreparedFilesystemCall::UnlinkAt {
                dirfd: a.fd()?,
                name: a.bytes()?,
                flags: a.i32()?,
            },
            FilesystemHostOperation::SetPermissions => PreparedFilesystemCall::SetPermissions {
                path: a.bytes()?,
                mode: a.u32()?,
            },
            FilesystemHostOperation::SetFilePermissions => {
                PreparedFilesystemCall::SetFilePermissions {
                    fd: a.fd()?,
                    mode: a.u32()?,
                }
            }
            FilesystemHostOperation::Rename => PreparedFilesystemCall::Rename {
                from: a.bytes()?,
                to: a.bytes()?,
            },
            FilesystemHostOperation::HardLink => PreparedFilesystemCall::HardLink {
                original: a.bytes()?,
                link: a.bytes()?,
            },
            FilesystemHostOperation::Symlink => PreparedFilesystemCall::Symlink {
                target: a.bytes()?,
                link: a.bytes()?,
            },
            FilesystemHostOperation::ReadLink => {
                let path = a.bytes()?;
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
                let path = a.bytes()?;
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
                link: a.bytes()?,
                existing: a.bytes()?,
                security_attributes: a.i64()?,
            },
            FilesystemHostOperation::OpenPathHandle => PreparedFilesystemCall::OpenPathHandle {
                path: a.bytes()?,
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
                let path = a.bytes()?;
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
                let path = a.bytes()?;
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
                path: a.bytes()?,
                uid: a.i32()?,
                gid: a.i32()?,
            },
            FilesystemHostOperation::ChangeOwnerNoFollow => {
                PreparedFilesystemCall::ChangeOwnerNoFollow {
                    path: a.bytes()?,
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
