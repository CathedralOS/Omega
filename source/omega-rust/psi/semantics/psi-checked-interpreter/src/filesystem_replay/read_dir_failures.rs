//! Exact replay for `read_dir` on an unknown compiler-owned descriptor.

use crate::{
    EvaluationObservations, FilesystemMutableByteOperand, FilesystemMutableByteOperandResolution,
    FilesystemMutableI64Operand, FilesystemMutableI64OperandResolution, FilesystemOperationAttempt,
    FilesystemReplay, FilesystemScalarOperand, FilesystemScalarOperandValue,
    FilesystemSourceInputReplayRecord,
};

use super::handle_failures::{
    unknown_descriptor_failure_attempt, unknown_descriptor_failure_has_exact_fixed_shape,
    unknown_handle_input_failure_replay_from_observations,
    unknown_handle_input_failure_replay_from_record,
};

const READ_DIR_OPERATION_TAG: u16 = 23;

/// Optional exact Source-input prefix followed by one `read_dir` call whose
/// directory descriptor deterministically fails with `EBADF`.
///
/// The requested count, complete mutable byte carrier, and mutable position
/// are exact authored inputs. The failed call leaves both carriers unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemInputUnknownDescriptorReadDirReplayRecord {
    source_input: Option<FilesystemSourceInputReplayRecord>,
    requested_count: u64,
    buffer: Vec<u8>,
    position: i64,
}

impl FilesystemInputUnknownDescriptorReadDirReplayRecord {
    pub fn new(
        source_input: Option<FilesystemSourceInputReplayRecord>,
        requested_count: u64,
        buffer: Vec<u8>,
        position: i64,
    ) -> Result<Self, String> {
        let requested_capacity = usize::try_from(requested_count)
            .map_err(|_| "filesystem replay read_dir request exceeds this host".to_owned())?;
        if requested_capacity > buffer.len() {
            return Err("filesystem replay read_dir request exceeds its mutable buffer".to_owned());
        }
        Ok(Self {
            source_input,
            requested_count,
            buffer,
            position,
        })
    }

    pub const fn source_input(&self) -> Option<&FilesystemSourceInputReplayRecord> {
        self.source_input.as_ref()
    }

    pub const fn requested_count(&self) -> u64 {
        self.requested_count
    }

    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    pub const fn position(&self) -> i64 {
        self.position
    }

    fn into_parts(self) -> (Option<FilesystemSourceInputReplayRecord>, u64, Vec<u8>, i64) {
        (
            self.source_input,
            self.requested_count,
            self.buffer,
            self.position,
        )
    }
}

impl FilesystemReplay {
    /// Construct the closed optional-Source plus one unknown-descriptor
    /// `read_dir` failure from typed compiler-owned evidence.
    pub fn from_input_unknown_descriptor_read_dir_record(
        record: FilesystemInputUnknownDescriptorReadDirReplayRecord,
    ) -> Result<Self, String> {
        let (source_input, requested_count, buffer, position) = record.into_parts();
        unknown_handle_input_failure_replay_from_record(
            source_input,
            unknown_descriptor_read_dir_attempt(requested_count, buffer, position),
            unknown_descriptor_read_dir_attempt_is_exact,
            "read_dir",
        )
    }

    /// Validate observed evidence for an optional Source-input prefix followed
    /// by exactly one `read_dir` failure on an unknown directory descriptor.
    pub fn from_input_unknown_descriptor_read_dir_observations(
        observations: &EvaluationObservations,
    ) -> Result<Self, String> {
        unknown_handle_input_failure_replay_from_observations(
            observations,
            unknown_descriptor_read_dir_attempt_is_exact,
            "read_dir",
        )
    }
}

pub(crate) fn unknown_descriptor_read_dir_from_exact_attempt(
    attempt: &FilesystemOperationAttempt,
) -> Option<(u64, &[u8], i64)> {
    let [
        FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(requested_count),
        },
    ] = attempt.scalar_operands.as_slice()
    else {
        return None;
    };
    let [buffer_resolution] = attempt.mutable_byte_operand_resolutions.as_slice() else {
        return None;
    };
    let [buffer] = attempt.mutable_byte_operands.as_slice() else {
        return None;
    };
    let [position_resolution] = attempt.mutable_i64_operand_resolutions.as_slice() else {
        return None;
    };
    let [position] = attempt.mutable_i64_operands.as_slice() else {
        return None;
    };
    let requested_count_is_valid =
        usize::try_from(*requested_count).is_ok_and(|count| count <= buffer_resolution.bytes.len());

    (buffer_resolution.operand_ordinal == 1
        && buffer.operand_ordinal == 1
        && buffer_resolution.bytes == buffer.pre_bytes
        && buffer_resolution.bytes == buffer.post_bytes
        && position_resolution.operand_ordinal == 3
        && position.operand_ordinal == 3
        && position_resolution.value == position.pre_value
        && position_resolution.value == position.post_value
        && unknown_descriptor_failure_has_exact_fixed_shape(attempt, READ_DIR_OPERATION_TAG)
        && requested_count_is_valid)
        .then_some((
            *requested_count,
            buffer_resolution.bytes.as_slice(),
            position_resolution.value,
        ))
}

pub(crate) fn unknown_descriptor_read_dir_attempt_is_exact(
    attempt: &FilesystemOperationAttempt,
) -> bool {
    unknown_descriptor_read_dir_from_exact_attempt(attempt).is_some()
}

pub(crate) fn unknown_descriptor_read_dir_attempt(
    requested_count: u64,
    buffer: Vec<u8>,
    position: i64,
) -> FilesystemOperationAttempt {
    debug_assert!(usize::try_from(requested_count).is_ok_and(|count| count <= buffer.len()));
    let resolution_buffer = buffer.clone();
    let pre_buffer = buffer.clone();
    let mut attempt = unknown_descriptor_failure_attempt(
        READ_DIR_OPERATION_TAG,
        vec![FilesystemScalarOperand {
            operand_ordinal: 2,
            value: FilesystemScalarOperandValue::U64(requested_count),
        }],
    );
    attempt.mutable_byte_operand_resolutions = vec![FilesystemMutableByteOperandResolution {
        operand_ordinal: 1,
        bytes: resolution_buffer,
    }];
    attempt.mutable_i64_operand_resolutions = vec![FilesystemMutableI64OperandResolution {
        operand_ordinal: 3,
        value: position,
    }];
    attempt.mutable_byte_operands = vec![FilesystemMutableByteOperand {
        operand_ordinal: 1,
        pre_bytes: pre_buffer,
        post_bytes: buffer,
    }];
    attempt.mutable_i64_operands = vec![FilesystemMutableI64Operand {
        operand_ordinal: 3,
        pre_value: position,
        post_value: position,
    }];
    attempt
}
