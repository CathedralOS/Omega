//!
//! This file owns the transfer limits and count checks.
//! `prepared_calls.rs` carries the prepared call and its argument cursor,
//! `prepared_outputs.rs` the prepared byte and integer outputs,
//! `logical_handle_plans.rs` the logical handle inputs, outputs and
//! retirements, and `tests.rs` the preparation tests.

mod logical_handle_plans;
mod prepared_calls;
mod prepared_outputs;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use logical_handle_plans::{
    FilesystemLogicalHandleResultSuccess, FilesystemLogicalHandleRetirementSuccess,
    PreparedFilesystemLogicalHandleInput, PreparedFilesystemLogicalHandleRetirement,
};
pub(crate) use logical_handle_plans::{
    PreparedFilesystemLogicalHandleOutput, PreparedFilesystemLogicalHandlePlan,
};
pub(crate) use prepared_calls::PreparedFilesystemCall;
pub(crate) use prepared_outputs::{PreparedByteOutput, PreparedTransferCount};

use super::{EvalResult, FilesystemHostOperation, Halt, trap};

pub(super) const MAX_FILESYSTEM_TRANSFER_BYTES: usize = 16 * 1024 * 1024;

const FILETIME_BYTES: usize = 8;

pub(super) const FIND_DATA_OUTPUT_BYTES: usize = 320;

const OVERLAPPED_BYTES: usize = 32;

const PATH_MAX_OUTPUT_BYTES: usize = 1024;

pub(super) const STAT_OUTPUT_BYTES: usize = crate::FILESYSTEM_METADATA_API_CARRIER_BYTES;

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

fn rooted_package_build_operation_refusal(
    operation: FilesystemHostOperation,
) -> Option<&'static str> {
    match operation {
        FilesystemHostOperation::Canonicalize => Some("would expose a host-absolute path"),
        FilesystemHostOperation::FindFirst
        | FilesystemHostOperation::FindNext
        | FilesystemHostOperation::FindClose => {
            Some("belongs to an unrooted find-cursor protocol not admitted by the Build facet")
        }
        _ => None,
    }
}
