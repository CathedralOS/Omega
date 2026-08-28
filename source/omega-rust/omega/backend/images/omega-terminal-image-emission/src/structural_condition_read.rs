//! Target-specific byte replay for structural Boolean condition reads.
//!
//! This module derives the active stack depth at each retained read and
//! reconstructs the exact x86-64 or AArch64 load-and-mask bytes. It does not
//! choose fields, layouts, or control flow.

use omega_calling_conventions::{IndirectPointerLocation, ValueLocation, ValuePlacement};
use omega_terminal_machine_code::{TerminalScalarStackEvidence, TerminalScalarStackMutationKind};

use super::instruction_loads::{
    aarch64_replay_memory_load, aarch64_replay_stack_load, aarch64_terminal_register,
    x86_replay_memory_load, x86_replay_rsp_load, x86_terminal_register,
};

pub(super) fn condition_stack_depth_before(
    evidence: &TerminalScalarStackEvidence,
    condition_start: usize,
    read_start: usize,
) -> Option<u32> {
    let mut depth = 0_u32;
    for mutation in evidence
        .mutations
        .iter()
        .filter(|mutation| mutation.offset >= condition_start && mutation.offset < read_start)
    {
        if mutation.offset.checked_add(mutation.byte_count)? > read_start {
            return None;
        }
        match mutation.kind {
            TerminalScalarStackMutationKind::Allocate { byte_size } => {
                depth = depth.checked_add(byte_size)?;
            }
            TerminalScalarStackMutationKind::Release { byte_size }
            | TerminalScalarStackMutationKind::X86ReleasePreservingFlags { byte_size } => {
                depth = depth.checked_sub(byte_size)?;
            }
            TerminalScalarStackMutationKind::X86Push => depth = depth.checked_add(8)?,
            TerminalScalarStackMutationKind::X86Pop => depth = depth.checked_sub(8)?,
        }
    }
    Some(depth)
}

pub(super) fn replay_x86_boolean_structural_read(
    placement: &ValuePlacement,
    field_byte_offset: u32,
    stack_depth: u32,
) -> Option<Vec<u8>> {
    if field_byte_offset >= u32::from(placement.shape.byte_size) {
        return None;
    }
    let mut bytes = Vec::new();
    if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
        let base = match *pointer {
            IndirectPointerLocation::Register(register) => {
                let base = x86_terminal_register(register)?;
                (base != 0).then_some(base)?
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                let incoming = stack_byte_offset.checked_add(8)?.checked_add(stack_depth)?;
                x86_replay_rsp_load(&mut bytes, 11, incoming, 8)?;
                11
            }
        };
        x86_replay_memory_load(&mut bytes, 0, base, field_byte_offset);
    } else {
        let location = placement.locations.iter().find(|location| match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => {
                let start = u32::from(*value_byte_offset);
                field_byte_offset >= start && field_byte_offset < start + u32::from(*byte_size)
            }
            ValueLocation::Indirect { .. } => false,
        })?;
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                ..
            } => {
                let register = x86_terminal_register(register)?;
                if register == 0 {
                    return None;
                }
                bytes.extend_from_slice(&[
                    0x48 | (((register >> 3) & 1) << 2),
                    0x89,
                    0xc0 | ((register & 7) << 3),
                ]);
                let shift = (field_byte_offset - u32::from(value_byte_offset)) * 8;
                if shift != 0 {
                    bytes.extend_from_slice(&[0x48, 0xc1, 0xe8, shift as u8]);
                }
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                ..
            } => {
                let incoming = stack_byte_offset
                    .checked_add(field_byte_offset - u32::from(value_byte_offset))?
                    .checked_add(8)?
                    .checked_add(stack_depth)?;
                x86_replay_rsp_load(&mut bytes, 0, incoming, 1)?;
            }
            ValueLocation::Indirect { .. } => return None,
        }
    }
    bytes.extend_from_slice(&[0x83, 0xe0, 0x01]);
    Some(bytes)
}

pub(super) fn replay_aarch64_boolean_structural_read(
    placement: &ValuePlacement,
    field_byte_offset: u32,
    stack_depth: u32,
) -> Option<Vec<u8>> {
    if field_byte_offset >= u32::from(placement.shape.byte_size) {
        return None;
    }
    let mut instructions = Vec::new();
    if let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() {
        let base = match *pointer {
            IndirectPointerLocation::Register(register) => {
                let base = aarch64_terminal_register(register)?;
                (base != 0).then_some(base)?
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => {
                let incoming = stack_depth.checked_add(stack_byte_offset)?;
                instructions.push(aarch64_replay_stack_load(9, incoming, 8)?);
                9
            }
        };
        instructions.push(aarch64_replay_memory_load(0, base, field_byte_offset)?);
    } else {
        let location = placement.locations.iter().find(|location| match location {
            ValueLocation::Register {
                value_byte_offset,
                byte_size,
                ..
            }
            | ValueLocation::Stack {
                value_byte_offset,
                byte_size,
                ..
            } => {
                let start = u32::from(*value_byte_offset);
                field_byte_offset >= start && field_byte_offset < start + u32::from(*byte_size)
            }
            ValueLocation::Indirect { .. } => false,
        })?;
        match *location {
            ValueLocation::Register {
                register,
                value_byte_offset,
                ..
            } => {
                let register = aarch64_terminal_register(register)?;
                if register == 0 {
                    return None;
                }
                let shift = (field_byte_offset - u32::from(value_byte_offset)) * 8;
                instructions.push(0xd340_fc00 | (shift << 16) | (u32::from(register) << 5));
            }
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                ..
            } => {
                let incoming = stack_depth
                    .checked_add(stack_byte_offset)?
                    .checked_add(field_byte_offset - u32::from(value_byte_offset))?;
                instructions.push(aarch64_replay_stack_load(0, incoming, 1)?);
            }
            ValueLocation::Indirect { .. } => return None,
        }
    }
    instructions.push(0x1200_0000);
    Some(
        instructions
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    )
}
