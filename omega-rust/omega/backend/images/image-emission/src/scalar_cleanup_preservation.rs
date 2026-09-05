//! Exact scalar-result preservation replay around affine cleanup.
//!
//! This module validates the retained save/restore frame and terminal return
//! bytes for scalar cleanup. It does not choose cleanup actions, frame layout,
//! or control-flow regions.

use machine_code::{
    ScalarCleanupPreservationEvidence, ScalarStackEvidence, UnitAffineCleanupRecord,
};
use semantic_vocabulary::MachineId;
use target::Architecture;

use super::ObjectError;
use super::unit_stack::validate_stack_adjustment_pair;

pub(super) fn validate_scalar_cleanup_preservation(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    stack: &ScalarStackEvidence,
    cleanup: Option<&UnitAffineCleanupRecord>,
) -> Result<(), ObjectError> {
    let (Some(cleanup), Some(preservation)) = (cleanup, stack.cleanup_preservation) else {
        return if cleanup.is_none() && stack.cleanup_preservation.is_none() {
            Ok(())
        } else {
            Err(ObjectError::InvalidUnitAffineCleanupEvidence(machine))
        };
    };
    validate_scalar_cleanup_preservation_record(
        architecture,
        machine,
        bytes,
        cleanup,
        preservation,
        bytes.len(),
    )
}

pub(super) fn validate_scalar_cleanup_preservation_record(
    architecture: Architecture,
    machine: MachineId,
    bytes: &[u8],
    cleanup: &UnitAffineCleanupRecord,
    preservation: ScalarCleanupPreservationEvidence,
    expected_end: usize,
) -> Result<(), ObjectError> {
    let invalid = || ObjectError::InvalidUnitAffineCleanupEvidence(machine);
    let exact_at = |offset: usize, expected: &[u8]| {
        bytes
            .get(offset..)
            .and_then(|tail| tail.get(..expected.len()))
            == Some(expected)
    };
    let frame = preservation.frame;
    let cleanup_end = cleanup
        .code_offset
        .checked_add(cleanup.byte_count)
        .ok_or_else(invalid)?;
    validate_stack_adjustment_pair(architecture, machine, None, bytes, frame)
        .map_err(|_| invalid())?;
    if cleanup_end != expected_end
        || frame.byte_size != 16
        || frame.allocation_offset != cleanup.code_offset
        || preservation.result_byte_offset != 0
    {
        return Err(invalid());
    }
    match architecture {
        Architecture::X86_64 => {
            let store = [0x48, 0x89, 0x44, 0x24, 0x00];
            let load = [0x48, 0x8b, 0x44, 0x24, 0x00];
            let allocation_end = frame
                .allocation_offset
                .checked_add(frame.allocation_byte_count)
                .ok_or_else(invalid)?;
            let load_end = preservation
                .result_load_offset
                .checked_add(load.len())
                .ok_or_else(invalid)?;
            let release_end = frame
                .release_offset
                .checked_add(frame.release_byte_count)
                .and_then(|end| end.checked_add(1))
                .ok_or_else(invalid)?;
            if preservation.aarch64_return_link.is_some()
                || preservation.result_store_offset != allocation_end
                || !exact_at(preservation.result_store_offset, &store)
                || load_end != frame.release_offset
                || !exact_at(preservation.result_load_offset, &load)
                || release_end != expected_end
                || bytes.get(expected_end.saturating_sub(1)) != Some(&0xc3)
            {
                return Err(invalid());
            }
        }
        Architecture::Aarch64 => {
            let Some(link) = preservation.aarch64_return_link else {
                return Err(invalid());
            };
            let result_store = 0xf900_03e0_u32.to_le_bytes();
            let link_store = 0xf900_07fe_u32.to_le_bytes();
            let result_load = 0xf940_03e0_u32.to_le_bytes();
            let link_load = 0xf940_07fe_u32.to_le_bytes();
            let allocation_store = frame.allocation_offset.checked_add(4).ok_or_else(invalid)?;
            let link_store_offset = preservation
                .result_store_offset
                .checked_add(4)
                .ok_or_else(invalid)?;
            let link_load_offset = preservation
                .result_load_offset
                .checked_add(4)
                .ok_or_else(invalid)?;
            let release_offset = link.load_offset.checked_add(4).ok_or_else(invalid)?;
            let terminal_end = frame.release_offset.checked_add(8).ok_or_else(invalid)?;
            if preservation.result_store_offset != allocation_store
                || !exact_at(preservation.result_store_offset, &result_store)
                || link.frame_byte_offset != 8
                || link.store_offset != link_store_offset
                || !exact_at(link.store_offset, &link_store)
                || !exact_at(preservation.result_load_offset, &result_load)
                || link.load_offset != link_load_offset
                || !exact_at(link.load_offset, &link_load)
                || frame.release_offset != release_offset
                || terminal_end != expected_end
                || !exact_at(
                    expected_end.saturating_sub(4),
                    &0xd65f_03c0_u32.to_le_bytes(),
                )
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
