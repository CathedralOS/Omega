use crate::{
    BuildIncludedSource, EvaluationObservations, FILESYSTEM_METADATA_API_CARRIER_BYTES,
    FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION, FilesystemByteOperand,
    FilesystemLogicalHandleInput, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind, FilesystemMutableByteOperand,
    FilesystemMutableByteOperandResolution, FilesystemObservationProvider,
    FilesystemOperationAttempt, FilesystemOperationAttemptOutcome, FilesystemOperationResult,
    FilesystemReplay, FilesystemScalarOperand, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayRecord, source_input_record_attempts,
    validate_filesystem_replay_size, validate_source_input_attempts,
};

const UNKNOWN_DESCRIPTOR_RESULT: i64 = -1;
const BAD_DESCRIPTOR_ERROR: i32 = 9;
const UNKNOWN_DESCRIPTOR_OSF_HANDLE_RESULT: i64 = -2;
const UNCHANGED_ERROR: i32 = 0;
const READ_OPERATION_TAG: u16 = 4;
const WRITE_OPERATION_TAG: u16 = 5;
const READ_AT_OPERATION_TAG: u16 = 6;
const WRITE_AT_OPERATION_TAG: u16 = 7;
const SEEK_OPERATION_TAG: u16 = 10;
const CLOSE_HANDLE_OPERATION_TAG: u16 = 29;
const GET_OSF_HANDLE_OPERATION_TAG: u16 = 30;
const FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG: u16 = 31;
const READ_FILE_METADATA_OPERATION_TAG: u16 = 39;
const SET_FILE_TIMES_OPERATION_TAG: u16 = 42;
const UNKNOWN_NATIVE_HANDLE_CLOSE_RESULT: i64 = 0;
const UNKNOWN_NATIVE_HANDLE_FINAL_PATH_RESULT: i64 = 0;
const INVALID_HANDLE_ERROR: i32 = 6;
const SET_FILE_TIMES_MINIMUM_CARRIER_BYTES: usize = 32;

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
    const fn operation_tag(self) -> u16 {
        match self {
            Self::Close => 8,
            Self::Sync => 43,
            Self::SyncData => 44,
            Self::Duplicate => 45,
        }
    }

    const fn from_operation_tag(operation_tag: u16) -> Option<Self> {
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

    fn into_parts(
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

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>, u64, u32) {
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

    fn into_source_input(self) -> Option<FilesystemSourceInputReplayRecord> {
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

    fn into_source_input(self) -> Option<FilesystemSourceInputReplayRecord> {
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

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, i64, i32) {
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
    const fn operation_tag(self) -> u16 {
        match self {
            Self::Sequential { .. } => READ_OPERATION_TAG,
            Self::Positioned { .. } => READ_AT_OPERATION_TAG,
        }
    }

    fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
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

    const fn count(self) -> u64 {
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

    fn into_parts(
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

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>) {
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
    const fn operation_tag(self) -> u16 {
        match self {
            Self::Sequential => WRITE_OPERATION_TAG,
            Self::Positioned { .. } => WRITE_AT_OPERATION_TAG,
        }
    }

    fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
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

    fn into_parts(
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
    const fn operation_tag(self) -> u16 {
        match self {
            Self::SetFilePermissions { .. } => 17,
            Self::SetLength { .. } => 41,
            Self::LockFile { .. } => 46,
            Self::ChangeFileOwner { .. } => 49,
        }
    }

    fn scalar_operands(self) -> Vec<FilesystemScalarOperand> {
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

    fn into_parts(
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

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, Vec<u8>) {
        (self.source_input, self.times)
    }
}

impl FilesystemReplay {
    /// Construct the closed optional-Source plus one unknown-descriptor
    /// operation rung from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_operation_record(
        record: FilesystemInputUnknownDescriptorOperationReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_operation_attempt(kind),
            unknown_descriptor_operation_attempt_is_exact,
            "operation",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one operand-free unknown-descriptor operation.
    pub fn from_input_unknown_descriptor_operation_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_operation_attempt_is_exact,
            "operation",
        )
    }

    /// Construct the closed optional-Source plus one unknown synthetic native
    /// handle `close_handle` failure from typed compiler-owned evidence.
    pub fn from_input_unknown_native_handle_close_handle_record(
        record: FilesystemInputUnknownNativeHandleCloseHandleReplayRecord,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_record(
            record.into_source_input(),
            unknown_native_handle_close_handle_attempt(),
            unknown_native_handle_close_handle_attempt_is_exact,
            "close_handle",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one failed close of an unknown compiler-owned synthetic native handle.
    pub fn from_input_unknown_native_handle_close_handle_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_native_handle_close_handle_attempt_is_exact,
            "close_handle",
        )
    }

    /// Construct the closed optional-Source plus one final-path failure on an
    /// unknown compiler-owned synthetic native handle from typed evidence.
    pub fn from_input_unknown_native_handle_final_path_name_by_handle_record(
        record: FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, buffer, capacity, flags) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_native_handle_final_path_name_by_handle_attempt(buffer, capacity, flags),
            unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
            "final_path_name_by_handle",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by the exact modeled final-path failure on an unknown synthetic handle.
    pub fn from_input_unknown_native_handle_final_path_name_by_handle_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
            "final_path_name_by_handle",
        )
    }

    /// Construct the closed optional-Source plus one unknown-descriptor
    /// `get_osfhandle` failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_get_osfhandle_record(
        record: FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_record(
            record.into_source_input(),
            unknown_descriptor_get_osfhandle_attempt(),
            unknown_descriptor_get_osfhandle_attempt_is_exact,
            "get_osfhandle",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one modeled fd-to-handle failure on an unknown descriptor.
    pub fn from_input_unknown_descriptor_get_osfhandle_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_get_osfhandle_attempt_is_exact,
            "get_osfhandle",
        )
    }

    /// Construct the closed optional-Source plus unknown-descriptor seek rung
    /// from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_seek_record(
        record: FilesystemInputUnknownDescriptorSeekReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, offset, whence) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_seek_attempt(offset, whence),
            unknown_descriptor_seek_attempt_is_exact,
            "seek",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one seek on an unknown descriptor.
    pub fn from_input_unknown_descriptor_seek_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_seek_attempt_is_exact,
            "seek",
        )
    }

    /// Construct the closed optional-Source plus one unknown-descriptor read
    /// failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_read_record(
        record: FilesystemInputUnknownDescriptorReadReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind, buffer) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_read_attempt(kind, buffer),
            unknown_descriptor_read_attempt_is_exact,
            "read",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one read failure on an unknown descriptor.
    pub fn from_input_unknown_descriptor_read_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_read_attempt_is_exact,
            "read",
        )
    }

    /// Construct the closed optional-Source plus one unknown-descriptor
    /// `read_file_metadata` failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_read_file_metadata_record(
        record: FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, carrier) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_read_file_metadata_attempt(carrier),
            unknown_descriptor_read_file_metadata_attempt_is_exact,
            "read_file_metadata",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one `read_file_metadata` failure on an unknown descriptor.
    pub fn from_input_unknown_descriptor_read_file_metadata_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_read_file_metadata_attempt_is_exact,
            "read_file_metadata",
        )
    }

    /// Construct the closed optional-Source plus one unknown-descriptor write
    /// failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_write_record(
        record: FilesystemInputUnknownDescriptorWriteReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind, payload) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_write_attempt(kind, payload),
            unknown_descriptor_write_attempt_is_exact,
            "write",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one write failure on an unknown descriptor.
    pub fn from_input_unknown_descriptor_write_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_write_attempt_is_exact,
            "write",
        )
    }

    /// Construct the closed optional-Source plus one write-gated scalar
    /// unknown-descriptor operation from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_write_operation_record(
        record: FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, kind) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_write_operation_attempt(kind),
            unknown_descriptor_write_operation_attempt_is_exact,
            "write operation",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one write-gated scalar operation on an unknown descriptor.
    pub fn from_input_unknown_descriptor_write_operation_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_write_operation_attempt_is_exact,
            "write operation",
        )
    }

    /// Construct the closed optional-Source plus one unknown-descriptor
    /// `set_file_times` failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_set_file_times_record(
        record: FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, times) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_set_file_times_attempt(times),
            unknown_descriptor_set_file_times_attempt_is_exact,
            "set_file_times",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by one `set_file_times` failure on an unknown descriptor.
    pub fn from_input_unknown_descriptor_set_file_times_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_set_file_times_attempt_is_exact,
            "set_file_times",
        )
    }
}

fn unknown_handle_input_failure_replay_from_record(
    source_input: Option<FilesystemSourceInputReplayRecord>,
    operation: FilesystemOperationAttempt,
    operation_is_exact: fn(&FilesystemOperationAttempt) -> bool,
    operation_name: &str,
) -> Result<FilesystemReplay, String> {
    let mut attempts = source_input.map_or_else(Vec::new, source_input_record_attempts);
    attempts.push(operation);
    validate_filesystem_replay_size(&attempts)?;
    let (operation, source_attempts) = attempts
        .split_last()
        .expect("typed unknown-descriptor failure record is nonempty");
    validate_unknown_descriptor_failure_attempts(
        source_attempts,
        operation,
        &[],
        operation_is_exact,
        operation_name,
    )?;
    Ok(FilesystemReplay {
        attempts: attempts.into(),
        expected_included_sources: std::sync::Arc::from([]),
    })
}

fn unknown_handle_input_failure_replay_from_observations(
    observations: &EvaluationObservations,
    operation_is_exact: fn(&FilesystemOperationAttempt) -> bool,
    operation_name: &str,
) -> Result<FilesystemReplay, String> {
    if observations.filesystem_operation_schema_version()
        != FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION
    {
        return Err("filesystem replay observation schema is not current".to_owned());
    }
    let attempts = observations.filesystem_operation_attempts();
    validate_filesystem_replay_size(attempts)?;
    let (operation, source_attempts) = attempts.split_last().ok_or_else(|| {
        format!("filesystem replay requires one failed unknown-descriptor {operation_name}")
    })?;
    validate_unknown_descriptor_failure_attempts(
        source_attempts,
        operation,
        observations.build_included_sources(),
        operation_is_exact,
        operation_name,
    )?;
    Ok(FilesystemReplay {
        attempts: attempts.to_vec().into(),
        expected_included_sources: std::sync::Arc::from([]),
    })
}

fn validate_unknown_descriptor_failure_attempts(
    source_attempts: &[FilesystemOperationAttempt],
    operation: &FilesystemOperationAttempt,
    included_sources: &[BuildIncludedSource],
    operation_is_exact: fn(&FilesystemOperationAttempt) -> bool,
    operation_name: &str,
) -> Result<(), String> {
    if !included_sources.is_empty() {
        return Err(format!(
            "filesystem replay failed unknown-descriptor {operation_name} cannot hand off generated sources"
        ));
    }
    if !source_attempts.is_empty() {
        validate_source_input_attempts(source_attempts)?;
    }
    if !operation_is_exact(operation) {
        return Err(format!(
            "filesystem replay failed unknown-descriptor {operation_name} lanes are inconsistent"
        ));
    }
    Ok(())
}

pub(crate) fn unknown_descriptor_operation_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<FilesystemInputUnknownDescriptorOperationReplayKind> {
    let kind = FilesystemInputUnknownDescriptorOperationReplayKind::from_operation_tag(
        attempt.operation_tag,
    )?;
    (attempt.scalar_operands.is_empty()
        && unknown_descriptor_failure_has_exact_common_shape(attempt, kind.operation_tag()))
    .then_some(kind)
}

fn unknown_descriptor_operation_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    unknown_descriptor_operation_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_operation_attempt(
    kind: FilesystemInputUnknownDescriptorOperationReplayKind,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(kind.operation_tag(), Vec::new())
}

pub(crate) fn unknown_descriptor_get_osfhandle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape_with_outcome(
            attempt,
            GET_OSF_HANDLE_OPERATION_TAG,
            UNKNOWN_DESCRIPTOR_OSF_HANDLE_RESULT,
            UNCHANGED_ERROR,
        )
}

pub(crate) fn unknown_descriptor_get_osfhandle_attempt() -> FilesystemOperationAttempt {
    let mut attempt = unknown_descriptor_failure_attempt(GET_OSF_HANDLE_OPERATION_TAG, Vec::new());
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_OSF_HANDLE_RESULT),
        post_error: UNCHANGED_ERROR,
    });
    attempt
}

pub(crate) fn unknown_native_handle_close_handle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    attempt.scalar_operands.is_empty()
        && attempt.byte_operands.is_empty()
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_handle_failure_has_exact_core_shape_with_outcome(
            attempt,
            CLOSE_HANDLE_OPERATION_TAG,
            UNKNOWN_NATIVE_HANDLE_CLOSE_RESULT,
            INVALID_HANDLE_ERROR,
            FilesystemLogicalHandleKind::Native,
        )
}

pub(crate) fn unknown_native_handle_close_handle_attempt() -> FilesystemOperationAttempt {
    let mut attempt = unknown_descriptor_failure_attempt(CLOSE_HANDLE_OPERATION_TAG, Vec::new());
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(UNKNOWN_NATIVE_HANDLE_CLOSE_RESULT),
        post_error: INVALID_HANDLE_ERROR,
    });
    attempt.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    attempt
}

pub(crate) fn unknown_native_handle_final_path_name_by_handle_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(&[u8], u64, u32)> {
    let [capacity, flags] = attempt.scalar_operands.as_slice() else {
        return None;
    };
    let (
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(capacity),
        },
        FilesystemScalarOperand {
            operand_ordinal: 3,
            value: FilesystemScalarOperandValue::U32(flags),
        },
    ) = (capacity, flags)
    else {
        return None;
    };
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    let capacity_on_host = usize::try_from(*capacity).ok()?;
    (attempt.byte_operands.is_empty()
        && resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && capacity_on_host <= resolution.bytes.len()
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_handle_failure_has_exact_core_shape_with_outcome(
            attempt,
            FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG,
            UNKNOWN_NATIVE_HANDLE_FINAL_PATH_RESULT,
            INVALID_HANDLE_ERROR,
            FilesystemLogicalHandleKind::Native,
        ))
    .then_some((resolution.bytes.as_slice(), *capacity, *flags))
}

pub(crate) fn unknown_native_handle_final_path_name_by_handle_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_native_handle_final_path_name_by_handle_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_native_handle_final_path_name_by_handle_attempt(
    buffer: Vec<u8>,
    capacity: u64,
    flags: u32,
) -> FilesystemOperationAttempt {
    let resolution_buffer = buffer.clone();
    let pre_buffer = buffer.clone();
    let mut attempt = unknown_descriptor_failure_attempt(
        FINAL_PATH_NAME_BY_HANDLE_OPERATION_TAG,
        vec![
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::U64(capacity),
            },
            FilesystemScalarOperand {
                operand_ordinal: 3,
                value: FilesystemScalarOperandValue::U32(flags),
            },
        ],
    );
    attempt.outcome = Some(FilesystemOperationAttemptOutcome::Returned {
        result: FilesystemOperationResult::Scalar(UNKNOWN_NATIVE_HANDLE_FINAL_PATH_RESULT),
        post_error: INVALID_HANDLE_ERROR,
    });
    attempt.logical_handle_inputs[0].kind = FilesystemLogicalHandleKind::Native;
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_buffer,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_buffer,
        post_bytes: buffer,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_seek_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(i64, i32)> {
    let [offset, whence] = attempt.scalar_operands.as_slice() else {
        return None;
    };
    let (
        FilesystemScalarOperand {
            operand_ordinal: 1,
            value: FilesystemScalarOperandValue::I64(offset),
        },
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::I32(whence),
        },
    ) = (offset, whence)
    else {
        return None;
    };
    unknown_descriptor_failure_has_exact_common_shape(attempt, SEEK_OPERATION_TAG)
        .then_some((*offset, *whence))
}

fn unknown_descriptor_seek_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    unknown_descriptor_seek_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_seek_attempt(
    offset: i64,
    whence: i32,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(
        SEEK_OPERATION_TAG,
        vec![
            FilesystemScalarOperand {
                operand_ordinal: 1,
                value: FilesystemScalarOperandValue::I64(offset),
            },
            FilesystemScalarOperand {
                operand_ordinal: 2,
                value: FilesystemScalarOperandValue::I32(whence),
            },
        ],
    )
}

pub(crate) fn unknown_descriptor_write_operation_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<FilesystemInputUnknownDescriptorWriteOperationReplayKind> {
    let kind = match (attempt.operation_tag, attempt.scalar_operands.as_slice()) {
        (
            17,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::U32(mode),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteOperationReplayKind::SetFilePermissions {
            mode: *mode,
        },
        (
            41,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I64(length),
                },
            ],
        ) => {
            FilesystemInputUnknownDescriptorWriteOperationReplayKind::SetLength { length: *length }
        }
        (
            46,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I32(operation),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteOperationReplayKind::LockFile {
            operation: *operation,
        },
        (
            49,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 1,
                    value: FilesystemScalarOperandValue::I32(uid),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I32(gid),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteOperationReplayKind::ChangeFileOwner {
            uid: *uid,
            gid: *gid,
        },
        _ => return None,
    };
    unknown_descriptor_failure_has_exact_common_shape(attempt, kind.operation_tag()).then_some(kind)
}

pub(crate) fn unknown_descriptor_read_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(FilesystemInputUnknownDescriptorReadReplayKind, &[u8])> {
    let kind = match (attempt.operation_tag, attempt.scalar_operands.as_slice()) {
        (
            READ_OPERATION_TAG,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(count),
                },
            ],
        ) => FilesystemInputUnknownDescriptorReadReplayKind::Sequential { count: *count },
        (
            READ_AT_OPERATION_TAG,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::U64(count),
                },
                FilesystemScalarOperand {
                    operand_ordinal: 3,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
            ],
        ) => FilesystemInputUnknownDescriptorReadReplayKind::Positioned {
            count: *count,
            offset: *offset,
        },
        _ => return None,
    };
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    let count = usize::try_from(kind.count()).ok()?;
    (resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && count <= resolution.bytes.len()
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_descriptor_failure_has_exact_base_shape(attempt, kind.operation_tag()))
    .then_some((kind, resolution.bytes.as_slice()))
}

fn unknown_descriptor_read_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    unknown_descriptor_read_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_read_attempt(
    kind: FilesystemInputUnknownDescriptorReadReplayKind,
    buffer: Vec<u8>,
) -> FilesystemOperationAttempt {
    let resolution_buffer = buffer.clone();
    let pre_buffer = buffer.clone();
    let mut attempt =
        unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands());
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_buffer,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_buffer,
        post_bytes: buffer,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_read_file_metadata_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<&[u8]> {
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    (attempt.scalar_operands.is_empty()
        && resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && resolution.bytes.len() >= FILESYSTEM_METADATA_API_CARRIER_BYTES
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_descriptor_failure_has_exact_base_shape(
            attempt,
            READ_FILE_METADATA_OPERATION_TAG,
        ))
    .then_some(resolution.bytes.as_slice())
}

fn unknown_descriptor_read_file_metadata_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_read_file_metadata_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_read_file_metadata_attempt(
    carrier: Vec<u8>,
) -> FilesystemOperationAttempt {
    let resolution_carrier = carrier.clone();
    let pre_carrier = carrier.clone();
    let mut attempt =
        unknown_descriptor_failure_attempt(READ_FILE_METADATA_OPERATION_TAG, Vec::new());
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_carrier,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_carrier,
        post_bytes: carrier,
    }];
    attempt
}

pub(crate) fn unknown_descriptor_write_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(FilesystemInputUnknownDescriptorWriteReplayKind, &[u8])> {
    let kind = match (attempt.operation_tag, attempt.scalar_operands.as_slice()) {
        (WRITE_OPERATION_TAG, []) => FilesystemInputUnknownDescriptorWriteReplayKind::Sequential,
        (
            WRITE_AT_OPERATION_TAG,
            [
                FilesystemScalarOperand {
                    operand_ordinal: 2,
                    value: FilesystemScalarOperandValue::I64(offset),
                },
            ],
        ) => FilesystemInputUnknownDescriptorWriteReplayKind::Positioned { offset: *offset },
        _ => return None,
    };
    let [payload] = attempt.byte_operands.as_slice() else {
        return None;
    };
    (payload.operand_ordinal == 1
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape(attempt, kind.operation_tag()))
    .then_some((kind, payload.bytes.as_slice()))
}

fn unknown_descriptor_write_attempt_is_exact(attempt: &FilesystemOperationAttempt) -> bool {
    unknown_descriptor_write_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_write_attempt(
    kind: FilesystemInputUnknownDescriptorWriteReplayKind,
    payload: Vec<u8>,
) -> FilesystemOperationAttempt {
    let mut attempt =
        unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands());
    attempt.byte_operands = vec![FilesystemByteOperand {
        operand_ordinal: 1,
        bytes: payload,
    }];
    attempt
}

fn unknown_descriptor_write_operation_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_write_operation_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_write_operation_attempt(
    kind: FilesystemInputUnknownDescriptorWriteOperationReplayKind,
) -> FilesystemOperationAttempt {
    unknown_descriptor_failure_attempt(kind.operation_tag(), kind.scalar_operands())
}

pub(crate) fn unknown_input_handle_failure_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_operation_attempt_is_exact(attempt)
        || unknown_native_handle_close_handle_attempt_is_exact(attempt)
        || unknown_native_handle_final_path_name_by_handle_attempt_is_exact(attempt)
        || unknown_descriptor_get_osfhandle_attempt_is_exact(attempt)
        || unknown_descriptor_seek_attempt_is_exact(attempt)
        || unknown_descriptor_read_attempt_is_exact(attempt)
        || unknown_descriptor_read_file_metadata_attempt_is_exact(attempt)
        || unknown_descriptor_write_attempt_is_exact(attempt)
        || unknown_descriptor_write_operation_attempt_is_exact(attempt)
        || unknown_descriptor_set_file_times_attempt_is_exact(attempt)
}

fn unknown_descriptor_failure_has_exact_common_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    unknown_descriptor_failure_has_exact_base_shape(attempt, operation_tag)
        && attempt.mutable_byte_operand_resolutions.is_empty()
        && attempt.mutable_byte_operands.is_empty()
}

fn unknown_descriptor_failure_has_exact_base_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    attempt.byte_operands.is_empty()
        && unknown_descriptor_failure_has_exact_core_shape(attempt, operation_tag)
}

fn unknown_descriptor_failure_has_exact_core_shape(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
) -> bool {
    unknown_descriptor_failure_has_exact_core_shape_with_outcome(
        attempt,
        operation_tag,
        UNKNOWN_DESCRIPTOR_RESULT,
        BAD_DESCRIPTOR_ERROR,
    )
}

fn unknown_descriptor_failure_has_exact_core_shape_with_outcome(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
    result: i64,
    post_error: i32,
) -> bool {
    unknown_handle_failure_has_exact_core_shape_with_outcome(
        attempt,
        operation_tag,
        result,
        post_error,
        FilesystemLogicalHandleKind::Descriptor,
    )
}

fn unknown_handle_failure_has_exact_core_shape_with_outcome(
    attempt: &FilesystemOperationAttempt,
    operation_tag: u16,
    result: i64,
    post_error: i32,
    logical_handle_kind: FilesystemLogicalHandleKind,
) -> bool {
    matches!(
        attempt,
        FilesystemOperationAttempt {
            operation_tag: observed_operation_tag,
            provider: FilesystemObservationProvider::RealScoped,
            outcome: Some(FilesystemOperationAttemptOutcome::Returned {
                result: FilesystemOperationResult::Scalar(observed_result),
                post_error: observed_post_error,
            }),
            scalar_operands: _,
            byte_operands: _,
            path_like_operands,
            rooted_path_operand_resolutions,
            returned_paths,
            observed_byte_regions,
            metadata_observations,
            mutable_byte_operand_resolutions: _,
            mutable_i64_operand_resolutions,
            mutable_byte_operands: _,
            mutable_i64_operands,
            authorized_paths,
            logical_handle_inputs,
            logical_handle_output: None,
            retired_logical_handles,
            grant_refusals,
        } if *observed_operation_tag == operation_tag
            && *observed_result == result
            && *observed_post_error == post_error
            && path_like_operands.is_empty()
            && rooted_path_operand_resolutions.is_empty()
            && returned_paths.is_empty()
            && observed_byte_regions.is_empty()
            && metadata_observations.is_empty()
            && mutable_i64_operand_resolutions.is_empty()
            && mutable_i64_operands.is_empty()
            && authorized_paths.is_empty()
            && matches!(
                logical_handle_inputs.as_slice(),
                [FilesystemLogicalHandleInput {
                    operand_ordinal: 0,
                    kind,
                    resolution: FilesystemLogicalHandleInputResolution::Unknown,
                }] if *kind == logical_handle_kind
            )
            && retired_logical_handles.is_empty()
            && grant_refusals.is_empty()
    )
}

pub(crate) fn unknown_descriptor_set_file_times_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<&[u8]> {
    let [resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [provider_carrier] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    (attempt.scalar_operands.is_empty()
        && resolution.operand_ordinal == 1
        && provider_carrier.operand_ordinal == 1
        && resolution.bytes.len() >= SET_FILE_TIMES_MINIMUM_CARRIER_BYTES
        && resolution.bytes == provider_carrier.pre_bytes
        && resolution.bytes == provider_carrier.post_bytes
        && unknown_descriptor_failure_has_exact_base_shape(attempt, SET_FILE_TIMES_OPERATION_TAG))
    .then_some(resolution.bytes.as_slice())
}

fn unknown_descriptor_set_file_times_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_set_file_times_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_set_file_times_attempt(
    times: Vec<u8>,
) -> FilesystemOperationAttempt {
    let resolution_times = times.clone();
    let pre_times = times.clone();
    let mut attempt = unknown_descriptor_failure_attempt(SET_FILE_TIMES_OPERATION_TAG, Vec::new());
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_times,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_times,
        post_bytes: times,
    }];
    attempt
}

fn unknown_descriptor_failure_attempt(
    operation_tag: u16,
    scalar_operands: Vec<FilesystemScalarOperand>,
) -> FilesystemOperationAttempt {
    FilesystemOperationAttempt {
        operation_tag,
        provider: FilesystemObservationProvider::RealScoped,
        outcome: Some(FilesystemOperationAttemptOutcome::Returned {
            result: FilesystemOperationResult::Scalar(UNKNOWN_DESCRIPTOR_RESULT),
            post_error: BAD_DESCRIPTOR_ERROR,
        }),
        scalar_operands,
        byte_operands: Vec::new(),
        path_like_operands: Vec::new(),
        rooted_path_operand_resolutions: Vec::new(),
        returned_paths: Vec::new(),
        observed_byte_regions: Vec::new(),
        metadata_observations: Vec::new(),
        mutable_byte_operand_resolutions: Vec::new(),
        mutable_i64_operand_resolutions: Vec::new(),
        mutable_byte_operands: Vec::new(),
        mutable_i64_operands: Vec::new(),
        authorized_paths: Vec::new(),
        logical_handle_inputs: vec![FilesystemLogicalHandleInput {
            operand_ordinal: 0,
            kind: FilesystemLogicalHandleKind::Descriptor,
            resolution: FilesystemLogicalHandleInputResolution::Unknown,
        }],
        logical_handle_output: None,
        retired_logical_handles: Vec::new(),
        grant_refusals: Vec::new(),
    }
}
