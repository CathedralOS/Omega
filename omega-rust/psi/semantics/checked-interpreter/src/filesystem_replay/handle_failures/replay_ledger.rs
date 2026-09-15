//! Recording unknown handle failures into the filesystem replay.

use crate::filesystem_replay::handle_failures::descriptor_attempts::{
    unknown_descriptor_read_attempt, unknown_descriptor_read_attempt_is_exact,
    unknown_descriptor_read_file_metadata_attempt,
    unknown_descriptor_read_file_metadata_attempt_is_exact, unknown_descriptor_seek_attempt,
    unknown_descriptor_seek_attempt_is_exact, unknown_descriptor_set_file_times_attempt,
    unknown_descriptor_set_file_times_attempt_is_exact, unknown_descriptor_write_attempt,
    unknown_descriptor_write_attempt_is_exact, unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_attempt_is_exact,
};
use crate::filesystem_replay::handle_failures::handle_replays::{
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_attempt_is_exact,
    unknown_handle_input_failure_replay_from_observations,
    unknown_handle_input_failure_replay_from_record,
};
use crate::filesystem_replay::handle_failures::native_handle_attempts::{
    unknown_descriptor_get_osfhandle_attempt, unknown_descriptor_get_osfhandle_attempt_is_exact,
    unknown_native_handle_close_handle_attempt,
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
};
use crate::filesystem_replay::handle_failures::{
    FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord,
    FilesystemInputUnknownDescriptorOperationReplayRecord,
    FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord,
    FilesystemInputUnknownDescriptorReadReplayRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    FilesystemInputUnknownDescriptorWriteReplayRecord,
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord,
};
use crate::{EvaluationObservations, FilesystemReplay};

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
