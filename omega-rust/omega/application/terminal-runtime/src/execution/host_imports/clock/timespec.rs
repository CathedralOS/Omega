//! Darwin and Linux timespecs use native geometry, not semantic field order.
//! The supported 64-bit platforms occupy two signed 64-bit words. Any authenticated,
//! aligned contiguous backing covering those 16 bytes can receive the write;
//! captured codecs and restrictions still decide whether it can commit. A
//! larger loan keeps its suffix. Accessed padding, pointers and invented storage reject
//! through the common borrowed-storage capture rather than acquiring values.

use std::sync::{Arc, Mutex};

use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::{
    TerminalBoundaryStorageBuffer, TerminalEffect, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

use super::super::borrowed_storage::{CapturedStorage, StorageTransfer};
use super::{ClockImplementation, ClockNamespace, TerminalClockSnapshot, TerminalRuntimeError};

pub(super) fn admit(
    signature: &TerminalImportSignature,
    layouts: &[NativeBorrowedStorageLayout],
    implementation: ClockImplementation,
) -> Result<NativeBorrowedStorageLayout, TerminalRuntimeError> {
    let invalid = || {
        TerminalRuntimeError::InvalidProvision(
            "clock timespec requires an admitted loan covering aligned native storage",
        )
    };
    let [parameter] = signature.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let layout = layouts
        .iter()
        .find(|layout| layout.structural_type == parameter.structural_type)
        .ok_or_else(invalid)?;
    let access_valid = match implementation {
        ClockImplementation::ReadTimespec => parameter.access == StructuralAccess::MutableBorrow,
        ClockImplementation::Nanosleep => matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ),
        _ => false,
    };
    if !access_valid
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || layout.byte_size < 16
        || layout.alignment < 8
    {
        return Err(invalid());
    }
    Ok(layout.clone())
}

pub(super) fn read(
    effect: &TerminalEffect,
    buffers: &mut [TerminalBoundaryStorageBuffer],
    signature: &TerminalImportSignature,
    layout: &NativeBorrowedStorageLayout,
    clock: &Arc<Mutex<TerminalClockSnapshot>>,
    namespace: ClockNamespace,
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    let invalid = || {
        TerminalEffectRejection::new("clock timespec operands differ from the installed contract")
    };
    let TerminalEffect::BoundaryCall {
        arguments,
        structural_arguments,
        byte_sequence_arguments,
        ..
    } = effect
    else {
        return Err(invalid());
    };
    let selector = super::selector(arguments, signature)?;
    if structural_arguments.len() != 1
        || structural_arguments[0].structural_type != layout.structural_type
        || !matches!(byte_sequence_arguments.as_slice(), [None])
    {
        return Err(invalid());
    }
    let mut storage = CapturedStorage::capture(
        buffers,
        StructuralAccess::MutableBorrow,
        Some(layout),
        16,
        StorageTransfer::ReplacePrefix,
    )?;
    let reading = clock
        .lock()
        .map_err(|_| TerminalEffectRejection::new("clock snapshot lock is poisoned"))?
        .reading(namespace, selector)?;
    let seconds = i64::try_from(reading / 1_000_000_000).map_err(|_| invalid())?;
    let nanoseconds = i64::try_from(reading % 1_000_000_000).map_err(|_| invalid())?;
    let result = super::status_result(signature, 0)?;
    storage
        .bytes
        .get_mut(..8)
        .ok_or_else(invalid)?
        .copy_from_slice(&seconds.to_le_bytes());
    storage
        .bytes
        .get_mut(8..16)
        .ok_or_else(invalid)?
        .copy_from_slice(&nanoseconds.to_le_bytes());
    // No clock mutation and no writeback until the complete replacement has
    // passed captured codecs/restrictions and the core authenticates endpoints.
    storage.stage(buffers, 16)?;
    Ok(result)
}

pub(super) fn sleep(
    effect: &TerminalEffect,
    buffers: &[TerminalBoundaryStorageBuffer],
    signature: &TerminalImportSignature,
    layout: &NativeBorrowedStorageLayout,
    clock: &Arc<Mutex<TerminalClockSnapshot>>,
    namespace: ClockNamespace,
) -> Result<TerminalEffectResult, TerminalEffectRejection> {
    let invalid =
        || TerminalEffectRejection::new("nanosleep operands differ from the installed contract");
    let TerminalEffect::BoundaryCall {
        arguments,
        structural_arguments,
        byte_sequence_arguments,
        ..
    } = effect
    else {
        return Err(invalid());
    };
    let [
        TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(remainder),
        },
    ] = arguments.as_slice()
    else {
        return Err(invalid());
    };
    if signature.scalar_parameters.as_slice()
        != [semantic_vocabulary::ScalarType::Integer(*scalar_type)]
        || !scalar_type.admits(IntegerValue::Unsigned(*remainder))
        || structural_arguments.len() != 1
        || structural_arguments[0].structural_type != layout.structural_type
        || !matches!(byte_sequence_arguments.as_slice(), [None])
    {
        return Err(invalid());
    }
    let parameter = signature
        .structural_parameters
        .first()
        .ok_or_else(invalid)?;
    let storage = CapturedStorage::capture(
        buffers,
        parameter.access,
        Some(layout),
        16,
        StorageTransfer::Observe,
    )?;
    let seconds = i64::from_le_bytes(
        storage
            .bytes
            .get(..8)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    );
    let nanoseconds = i64::from_le_bytes(
        storage
            .bytes
            .get(8..16)
            .ok_or_else(invalid)?
            .try_into()
            .map_err(|_| invalid())?,
    );
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        // A raw Linux error has no libc state to establish. Darwin's error
        // path sets errno and applies libc's internal conversions, neither of
        // which is licensed by an elapsed/epoch snapshot. Returning Linux's
        // -EINVAL there, or guessing a C error without its model, is not replay.
        if matches!(namespace, ClockNamespace::Darwin) {
            return Err(TerminalEffectRejection::new(
                "Darwin nanosleep error requires a hosted errno model",
            ));
        }
        // Invalid requests advance no clock and modify neither carrier.
        return super::status_result(signature, -22);
    }
    let duration = u64::try_from(seconds)
        .ok()
        .and_then(|seconds| seconds.checked_mul(1_000_000_000))
        .and_then(|seconds| {
            u64::try_from(nanoseconds)
                .ok()
                .and_then(|nanoseconds| seconds.checked_add(nanoseconds))
        })
        .ok_or_else(|| {
            TerminalEffectRejection::new("nanosleep duration exceeds modeled nanoseconds")
        })?;
    // Linux kernel/time/hrtimer.c and Apple Libc gen/nanosleep.c write the
    // remainder on interruption, not success. This signal-free model completes
    // without accessing it, even if its numerical carrier is nonzero. Requiring null or
    // manufacturing remainder backing would invent permission/preconditions.
    // The borrowed request is read only; no replacements are staged.
    super::wait(clock, signature, duration)
}
