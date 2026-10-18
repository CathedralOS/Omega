//! Move an admitted x64 call onto image-owned storage before any adapter frame.
//!
//! The ordinary ABI bridge remains unchanged. Stack arguments are copied by
//! their selected placements, not by a firmware signature or parameter count.
//! Registers used by this prefix are saved before use and restored before the
//! bridge call. On return only rsp is restored: the bridge's result survives.
//! These saved slots belong to the single physical image-entry activation;
//! this is not a reentrant callable trampoline or an authority to grant roots.

use arena::Arena;
use calling_conventions::{CallPlan, EntryControl, IndirectPointerLocation, ValueLocation};
use diagnostics::Diagnostic;
use machine_emission::native_entry_bridge::NativeEntryBridge;

use super::invalid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Destination {
    #[default]
    SavedStack,
    SavedAccumulator,
    SavedScratch,
    StackTop,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct Fixup {
    pub field: usize,
    pub destination: Destination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StackSwitch {
    pub bytes: Vec<u8>,
    pub fixups: Arena<Fixup>,
    pub outgoing_frame_bytes: u32,
}

impl StackSwitch {
    /// The wrapper's live call frame, including the return address pushed by
    /// its call into the ordinary ABI bridge (whose frame is separate).
    pub fn stack_overhead_bytes(&self) -> u64 {
        u64::from(self.outgoing_frame_bytes) + 8
    }
}

fn incoming_stack_piece(location: &ValueLocation) -> Option<(u32, u32)> {
    match *location {
        ValueLocation::Stack {
            stack_byte_offset,
            byte_size,
            ..
        } => Some((stack_byte_offset, byte_size)),
        ValueLocation::Indirect {
            pointer:
                IndirectPointerLocation::Stack {
                    stack_byte_offset, ..
                },
            ..
        } => Some((stack_byte_offset, 8)),
        _ => None,
    }
}

pub(super) fn emit(bridge: &NativeEntryBridge) -> Result<StackSwitch, Diagnostic> {
    let inbound: &CallPlan = &bridge.request().inbound;
    let alignment = u32::from(inbound.stack_alignment);
    if inbound.policy.architecture() != target::Architecture::X86_64
        || inbound.entry_control != EntryControl::CallReturn
        || alignment < 16
        || !alignment.is_power_of_two()
    {
        return Err(invalid("private entry stack requires an aligned x64 call"));
    }
    // An indirect result pointer is an input even though it occurs in the
    // result plan. Direct result placements are outputs and are never read.
    let stack_pieces = inbound
        .parameters
        .iter()
        .flat_map(|value| &value.locations)
        .chain(
            inbound
                .result
                .iter()
                .flat_map(|value| &value.locations)
                .filter(|location| matches!(location, ValueLocation::Indirect { .. })),
        )
        .filter_map(incoming_stack_piece)
        .collect::<Vec<_>>();
    let mut frame_end = u32::from(inbound.shadow_bytes);
    for &(offset, byte_count) in &stack_pieces {
        frame_end = frame_end.max(
            offset
                .checked_add(byte_count)
                .ok_or_else(|| invalid("incoming stack argument extent overflows"))?,
        );
    }
    let outgoing_frame_bytes = frame_end
        .checked_add(alignment - 1)
        .map(|bytes| bytes & !(alignment - 1))
        .filter(|bytes| *bytes <= i32::MAX as u32 - 8)
        .ok_or_else(|| invalid("private entry call frame exceeds x64 displacement range"))?;
    let mut bytes = Vec::new();
    let mut fixups = Arena::new();
    let mut reference = |bytes: &mut Vec<u8>, opcode: &[u8], destination| {
        bytes.extend_from_slice(opcode);
        fixups.insert(Fixup {
            field: bytes.len(),
            destination,
        });
        bytes.extend_from_slice(&[0; 4]);
    };
    reference(&mut bytes, &[0x48, 0x89, 0x25], Destination::SavedStack);
    reference(
        &mut bytes,
        &[0x48, 0x89, 0x05],
        Destination::SavedAccumulator,
    );
    reference(&mut bytes, &[0x4c, 0x89, 0x15], Destination::SavedScratch);
    bytes.extend_from_slice(&[0x49, 0x89, 0xe2]); // mov r10, rsp
    reference(&mut bytes, &[0x48, 0x8d, 0x25], Destination::StackTop);
    bytes.extend_from_slice(&[0x48, 0x81, 0xec]); // sub rsp, outgoing frame
    bytes.extend_from_slice(&outgoing_frame_bytes.to_le_bytes());
    for (offset, byte_count) in stack_pieces {
        let mut copied = 0;
        while copied < byte_count {
            let remaining = byte_count - copied;
            let width = if remaining >= 8 {
                8
            } else if remaining >= 4 {
                4
            } else if remaining >= 2 {
                2
            } else {
                1
            };
            // Input placements are caller-stack-relative, excluding the
            // return address at the saved arrival rsp.
            let destination = offset + copied;
            let source = destination + 8;
            match width {
                8 => bytes.extend_from_slice(&[0x49, 0x8b, 0x82]),
                4 => bytes.extend_from_slice(&[0x41, 0x8b, 0x82]),
                2 => bytes.extend_from_slice(&[0x41, 0x0f, 0xb7, 0x82]),
                _ => bytes.extend_from_slice(&[0x41, 0x0f, 0xb6, 0x82]),
            }
            bytes.extend_from_slice(&source.to_le_bytes());
            match width {
                8 => bytes.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]),
                4 => bytes.extend_from_slice(&[0x89, 0x84, 0x24]),
                2 => bytes.extend_from_slice(&[0x66, 0x89, 0x84, 0x24]),
                _ => bytes.extend_from_slice(&[0x88, 0x84, 0x24]),
            }
            bytes.extend_from_slice(&destination.to_le_bytes());
            copied += width;
        }
    }
    reference(
        &mut bytes,
        &[0x48, 0x8b, 0x05],
        Destination::SavedAccumulator,
    );
    reference(&mut bytes, &[0x4c, 0x8b, 0x15], Destination::SavedScratch);
    bytes.push(0xe8);
    let bridge_call_field = bytes.len();
    bytes.extend_from_slice(&[0; 4]);
    reference(&mut bytes, &[0x48, 0x8b, 0x25], Destination::SavedStack);
    bytes.push(0xc3);
    let bridge_offset = bytes.len();
    let distance = i32::try_from(bridge_offset - (bridge_call_field + 4))
        .map_err(|_| invalid("private entry bridge is outside rel32 range"))?;
    bytes[bridge_call_field..bridge_call_field + 4].copy_from_slice(&distance.to_le_bytes());
    fixups.insert(Fixup {
        field: bridge_offset + bridge.call_field_offset(),
        destination: Destination::Body,
    });
    bytes.extend_from_slice(bridge.bytes());
    Ok(StackSwitch {
        bytes,
        fixups,
        outgoing_frame_bytes,
    })
}
