//!
//! This file owns the failure result constants and tags.
//! `unknown_descriptor_records.rs` carries the replay records,
//! `replay_ledger.rs` records the failures into the filesystem replay,
//! `handle_replays.rs` replays recorded failures, `native_handle_attempts.rs`
//! recognizes native handle attempts and `descriptor_attempts.rs`
//! recognizes descriptor attempts and their exact shapes.

mod descriptor_attempts;
mod handle_replays;
mod native_handle_attempts;
mod replay_ledger;
mod unknown_descriptor_records;

pub(crate) use descriptor_attempts::{
    unknown_descriptor_failure_attempt, unknown_descriptor_failure_has_exact_core_shape,
    unknown_descriptor_failure_has_exact_fixed_shape,
    unknown_input_handle_failure_attempt_is_exact,
};
#[cfg(test)]
pub(crate) use descriptor_attempts::{
    unknown_descriptor_read_attempt, unknown_descriptor_read_file_metadata_attempt,
    unknown_descriptor_read_file_metadata_from_exact_attempt,
    unknown_descriptor_read_from_exact_attempt, unknown_descriptor_seek_attempt,
    unknown_descriptor_seek_from_exact_attempt, unknown_descriptor_set_file_times_attempt,
    unknown_descriptor_set_file_times_from_exact_attempt, unknown_descriptor_write_attempt,
    unknown_descriptor_write_from_exact_attempt, unknown_descriptor_write_operation_attempt,
    unknown_descriptor_write_operation_from_exact_attempt,
};
pub(crate) use handle_replays::{
    unknown_descriptor_operation_attempt, unknown_descriptor_operation_from_exact_attempt,
    unknown_handle_input_failure_replay_from_observations,
    unknown_handle_input_failure_replay_from_record,
};
#[cfg(test)]
pub(crate) use native_handle_attempts::{
    unknown_descriptor_get_osfhandle_attempt, unknown_descriptor_get_osfhandle_attempt_is_exact,
    unknown_native_handle_close_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_attempt,
    unknown_native_handle_final_path_name_by_handle_from_exact_attempt,
};
pub(crate) use native_handle_attempts::{
    unknown_native_handle_close_handle_attempt_is_exact,
    unknown_native_handle_final_path_name_by_handle_attempt_is_exact,
};
pub use unknown_descriptor_records::{
    FilesystemInputUnknownDescriptorGetOsfHandleReplayRecord,
    FilesystemInputUnknownDescriptorOperationReplayKind,
    FilesystemInputUnknownDescriptorOperationReplayRecord,
    FilesystemInputUnknownDescriptorReadFileMetadataReplayRecord,
    FilesystemInputUnknownDescriptorReadReplayKind,
    FilesystemInputUnknownDescriptorReadReplayRecord,
    FilesystemInputUnknownDescriptorSeekReplayRecord,
    FilesystemInputUnknownDescriptorSetFileTimesReplayRecord,
    FilesystemInputUnknownDescriptorWriteOperationReplayKind,
    FilesystemInputUnknownDescriptorWriteOperationReplayRecord,
    FilesystemInputUnknownDescriptorWriteReplayKind,
    FilesystemInputUnknownDescriptorWriteReplayRecord,
    FilesystemInputUnknownNativeHandleCloseHandleReplayRecord,
    FilesystemInputUnknownNativeHandleFinalPathNameByHandleReplayRecord,
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
