//! Darwin timespec writes use native byte geometry, not semantic field order.
//! The SDK's time_t and long occupy two signed 64-bit words. Any authenticated,
//! aligned contiguous backing covering those 16 bytes can receive the write;
//! captured codecs and restrictions still decide whether it can commit. A
//! larger loan keeps its suffix. Accessed padding, pointers and invented storage reject
//! through the common borrowed-storage capture rather than acquiring values.

use std::sync::{Arc, Mutex};

use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::{
    TerminalBoundaryScalarBuffer, TerminalEffect, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

use super::super::borrowed_storage::CapturedStorage;
use super::{TerminalClockSnapshot, TerminalRuntimeError};

pub(super) fn admit(
    signature: &TerminalImportSignature,
    layouts: &[NativeBorrowedStorageLayout],
) -> Result<NativeBorrowedStorageLayout, TerminalRuntimeError> {
    let invalid = || {
        TerminalRuntimeError::InvalidProvision(
            "clock timespec requires an admitted mutable loan covering aligned native storage",
        )
    };
    let [parameter] = signature.structural_parameters.as_slice() else {
        return Err(invalid());
    };
    let layout = layouts
        .iter()
        .find(|layout| layout.structural_type == parameter.structural_type)
        .ok_or_else(invalid)?;
    if parameter.access != StructuralAccess::MutableBorrow
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
    buffers: &mut [TerminalBoundaryScalarBuffer],
    signature: &TerminalImportSignature,
    layout: &NativeBorrowedStorageLayout,
    clock: &Arc<Mutex<TerminalClockSnapshot>>,
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
    let [
        TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(selector),
        },
    ] = arguments.as_slice()
    else {
        return Err(invalid());
    };
    if signature.scalar_parameters.as_slice()
        != [semantic_vocabulary::ScalarType::Integer(*scalar_type)]
        || !scalar_type.admits(IntegerValue::Unsigned(*selector))
        || structural_arguments.len() != 1
        || structural_arguments[0].structural_type != layout.structural_type
        || !matches!(byte_sequence_arguments.as_slice(), [None])
    {
        return Err(invalid());
    }
    let mut storage =
        CapturedStorage::capture(buffers, StructuralAccess::MutableBorrow, Some(layout), 16)?;
    let reading = clock
        .lock()
        .map_err(|_| TerminalEffectRejection::new("clock snapshot lock is poisoned"))?
        .reading(*selector)?;
    let seconds = i64::try_from(reading / 1_000_000_000).map_err(|_| invalid())?;
    let nanoseconds = i64::try_from(reading % 1_000_000_000).map_err(|_| invalid())?;
    let result_type = signature.result.scalar().ok_or_else(invalid)?;
    let semantic_vocabulary::ScalarType::Integer(result_type) = result_type else {
        return Err(invalid());
    };
    let result = TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
        scalar_type: result_type,
        value: IntegerValue::Signed(0),
    });
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
    storage.stage(buffers)?;
    Ok(result)
}
