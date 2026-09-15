//! Replay records for unknown descriptor and native handle failures.

use crate::filesystem_replay::handle_failures::{
    READ_AT_OPERATION_TAG, READ_OPERATION_TAG, SET_FILE_TIMES_MINIMUM_CARRIER_BYTES,
    WRITE_AT_OPERATION_TAG, WRITE_OPERATION_TAG,
};
use crate::{
    FilesystemScalarOperand, FilesystemScalarOperandValue, FilesystemSourceInputReplayRecord,
};

/// One operand-free descriptor operation whose unknown input deterministically
/// fails with `EBADF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemInputUnknownDescriptorOperationReplayKind {
    Close,
    Sync,
    SyncData,
    Duplicate,
}

impl FilesystemInputUnknownDescriptorOperationReplayKind {
    pub(crate) const fn operation_tag(self) -> u16 {
        match self {
            Self::Close => 8,
            Self::Sync => 43,
            Self::SyncData => 44,
            Self::Duplicate => 45,
        }
    }

    pub(crate) const fn from_operation_tag(operation_tag: u16) -> Option<Self> {
        match operation_tag {
            8 => Some(Self::Close),
            43 => Some(Self::Sync),
            44 => Some(Self::SyncData),
            45 => Some(Self::Duplicate),
            _ => None,
        }
    }
}

/// Optional Source-input prefix followed by exactly one operand-free operation
/// on an unknown descriptor.
///
/// The selected operation contributes no authored coordinates to this record:
/// its provider, result, error, logical input, and empty side lanes are fixed by
/// the record type. In particular, the raw provider descriptor is not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorOperationReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownDescriptorOperationReplayKind,
}

impl FilesystemInputUnknownDescriptorOperationReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownDescriptorOperationReplayKind,
    ) -> Self {
        Self { source_input, kind }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> FilesystemInputUnknownDescriptorOperationReplayKind {
        self.kind
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownDescriptorOperationReplayKind,
    ) {
        (self.source_input, self.kind)
    }
}

/// Optional exact Source-input prefix followed by one modeled fd-to-handle
/// bridge call on an unknown descriptor.
///
/// The operation has no caller-authored coordinates beyond the optional
/// prefix. Its synthetic `-2` result and empty handle-output lane are fixed by
/// this record type; no provider descriptor or operating-system handle is
/// retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
}

/// Optional exact Source-input prefix followed by one failed close of an
/// unknown compiler-owned synthetic native handle.
///
/// The fixed result and error describe only Omega's evaluator model. No host
/// operating-system handle or handle authority is retained by this record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownNativeHandleCloseHandleReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
}

/// Optional exact Source-input prefix followed by one failed final-path query
/// on an unknown compiler-owned synthetic native handle.
///
/// The authored buffer, capacity, and flags are retained. The fixed result and
/// error describe only Omega's compiler-owned synthetic handle model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    buffer: Vec<u8>,
    capacity: u64,
    flags: u32,
}

impl FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        buffer: Vec<u8>,
        capacity: u64,
        flags: u32,
    ) -> Result<Self, String> {
        let capacity_on_host = usize::try_from(capacity).map_err(|_| {
            "filesystem replay final_path_name_by_handle capacity exceeds this host".to_owned()
        })?;
        if capacity_on_host > buffer.len() {
            return Err(
                "filesystem replay final_path_name_by_handle capacity exceeds its mutable buffer"
                    .to_owned(),
            );
        }
        Ok(Self {
            source_input,
            buffer,
            capacity,
            flags,
        })
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub const fn capacity(&self) -> u64 {
        self.capacity
    }

    pub const fn flags(&self) -> u32 {
        self.flags
    }

    pub(crate) fn into_parts(
        self,
    ) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>, u64, u32) {
        (self.source_input, self.buffer, self.capacity, self.flags)
    }
}

impl FilesystemInputUnknownNativeHandleCloseHandleReplayRecord {
    pub fn new(source_input: Option<FilesystemSourceInputReplayRecord>) -> Self {
        Self { source_input }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub(crate) fn into_source_input(self) -> Option<FilesystemSourceInputReplayRecord> {
        self.source_input
    }
}

impl FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord {
    pub fn new(source_input: Option<FilesystemSourceInputReplayRecord>) -> Self {
        Self { source_input }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub(crate) fn into_source_input(self) -> Option<FilesystemSourceInputReplayRecord> {
        self.source_input
    }
}

/// Optional Source-input prefix followed by exactly one seek on an unknown
/// descriptor.
///
/// Only the authored seek coordinates survive in this record. The operation
/// tag, scoped provider, failed result, error, unknown descriptor input, and
/// empty side lanes are fixed by the record type; no provider descriptor is
/// retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorSeekReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    offset: i64,
    whence: i32,
}

impl FilesystemInputUnknownDescriptorSeekReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        offset: i64,
        whence: i32,
    ) -> Self {
        Self {
            source_input,
            offset,
            whence,
        }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn offset(&self) -> i64 {
        self.offset
    }

    pub const fn whence(&self) -> i32 {
        self.whence
    }

    pub(crate) fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, i64, i32) {
        (self.source_input, self.offset, self.whence)
    }
}

/// One mutable-buffer read whose unknown descriptor deterministically fails
/// with `EBADF`. Each variant retains only its authored scalar coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemInputUnknownDescriptorReadReplayKind {
    Sequential { count: u64 },
    Positioned { count: u64, offset: i64 },
}

impl FilesystemInputUnknownDescriptorReadReplayKind {
    pub(crate) const fn operation_tag(self) -> u16 {
        match self {
            Self::Sequential { .. } => READ_OPERATION_TAG,
            Self::Positioned { .. } => READ_AT_OPERATION_TAG,
        }
    }

    pub(crate) fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
        match self {
            Self::Sequential { count } => vec![FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::U64(count),
            }],
            Self::Positioned { count, offset } => vec![
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(count),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 3,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
            ],
        }
    }

    pub(crate) const fn count(self) -> u64 {
        match self {
            Self::Sequential { count } | Self::Positioned { count, .. } => count,
        }
    }
}

/// Optional exact Source-input prefix followed by one read whose unknown
/// descriptor deterministically fails with `EBADF`.
///
/// The exact authored mutable buffer is retained once here. Replay rebuilds
/// its equal resolution and provider-visible pre/post states without retaining
/// or consulting a filesystem provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorReadReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownDescriptorReadReplayKind,
    buffer: Vec<u8>,
}

impl FilesystemInputUnknownDescriptorReadReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownDescriptorReadReplayKind,
        buffer: Vec<u8>,
    ) -> Result<Self, String> {
        let count = usize::try_from(kind.count())
            .map_err(|_| "filesystem replay unknown-descriptor read count exceeds this host")?;
        if count > buffer.len() {
            return Err(
                "filesystem replay unknown-descriptor read count exceeds its mutable buffer"
                    .to_owned(),
            );
        }
        Ok(Self {
            source_input,
            kind,
            buffer,
        })
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> FilesystemInputUnknownDescriptorReadReplayKind {
        self.kind
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownDescriptorReadReplayKind,
        Vec<u8>,
    ) {
        (self.source_input, self.kind, self.buffer)
    }
}

/// Optional exact Source-input prefix followed by one `read_file_metadata`
/// whose unknown descriptor deterministically fails with `EBADF`.
///
/// The complete authored mutable carrier is retained once here. Replay
/// reconstructs equal resolution, provider-visible pre-state, and
/// provider-visible post-state without retaining or consulting a filesystem
/// provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    carrier: Vec<u8>,
}

impl FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord {
    pub fn new(source_input: Option<FilesystemSourceInputReplayRecord>, carrier: Vec<u8>) -> Self {
        Self {
            source_input,
            carrier,
        }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub fn carrier(&self) -> &[u8] {
        &self.carrier
    }

    pub(crate) fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>) {
        (self.source_input, self.carrier)
    }
}

/// One immutable-payload write whose unknown descriptor deterministically
/// fails with `EBADF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemInputUnknownDescriptorWriteReplayKind {
    Sequential,
    Positioned { offset: i64 },
}

impl FilesystemInputUnknownDescriptorWriteReplayKind {
    pub(crate) const fn operation_tag(self) -> u16 {
        match self {
            Self::Sequential => WRITE_OPERATION_TAG,
            Self::Positioned { .. } => WRITE_AT_OPERATION_TAG,
        }
    }

    pub(crate) fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
        match self {
            Self::Sequential => Vec::new(),
            Self::Positioned { offset } => vec![FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I64(offset),
            }],
        }
    }
}

/// Optional exact Source-input prefix followed by one write whose unknown
/// descriptor deterministically fails with `EBADF`.
///
/// The exact authored immutable payload is retained directly. Its length is
/// neither represented by a synthetic scalar nor inferred from an Output tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorWriteReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownDescriptorWriteReplayKind,
    payload: Vec<u8>,
}

impl FilesystemInputUnknownDescriptorWriteReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownDescriptorWriteReplayKind,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            source_input,
            kind,
            payload,
        }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> FilesystemInputUnknownDescriptorWriteReplayKind {
        self.kind
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownDescriptorWriteReplayKind,
        Vec<u8>,
    ) {
        (self.source_input, self.kind, self.payload)
    }
}

/// One write-gated scalar operation whose unknown descriptor deterministically
/// fails with `EBADF`. Each variant retains only its authored scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemInputUnknownDescriptorWriteOperationReplayKind {
    SetFilePermissions { mode: u32 },
    SetLength { length: i64 },
    LockFile { operation: i32 },
    ChangeFileOwner { uid: i32, gid: i32 },
}

impl FilesystemInputUnknownDescriptorWriteOperationReplayKind {
    pub(crate) const fn operation_tag(self) -> u16 {
        match self {
            Self::SetFilePermissions { .. } => 17,
            Self::SetLength { .. } => 41,
            Self::LockFile { .. } => 46,
            Self::ChangeFileOwner { .. } => 49,
        }
    }

    pub(crate) fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
        match self {
            Self::SetFilePermissions { mode } => vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::U32(mode),
            }],
            Self::SetLength { length } => vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I64(length),
            }],
            Self::LockFile { operation } => vec![FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I32(operation),
            }],
            Self::ChangeFileOwner { uid, gid } => vec![
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I32(uid),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I32(gid),
                },
            ],
        }
    }
}

/// Optional exact Source-input prefix followed by one closed write-gated
/// scalar operation on an unknown descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorWriteOperationReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
}

impl FilesystemInputUnknownDescriptorWriteOperationReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    ) -> Self {
        Self { source_input, kind }
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn kind(&self) -> FilesystemInputUnknownDescriptorWriteOperationReplayKind {
        self.kind
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Option<FilesystemSourceInputReplayRecord>,
        FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    ) {
        (self.source_input, self.kind)
    }
}

/// Optional exact Source-input prefix followed by one `set_file_times` call
/// whose unknown descriptor deterministically fails with `EBADF`.
///
/// The exact authored times carrier is retained once here. Replay reconstructs
/// its equal resolution, provider-visible pre-state, and provider-visible
/// post-state without retaining or consulting a filesystem provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorSetFileTimesReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    times: Vec<u8>,
}

impl FilesystemInputUnknownDescriptorSetFileTimesReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        times: Vec<u8>,
    ) -> Result<Self, String> {
        if times.len() < SET_FILE_TIMES_MINIMUM_CARRIER_BYTES {
            return Err(format!(
                "filesystem replay failed unknown-descriptor set_file_times carrier is shorter than {SET_FILE_TIMES_MINIMUM_CARRIER_BYTES} bytes"
            ));
        }
        Ok(Self {
            source_input,
            times,
        })
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub fn times(&self) -> &[u8] {
        &self.times
    }

    pub(crate) fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>) {
        (self.source_input, self.times)
    }
}
