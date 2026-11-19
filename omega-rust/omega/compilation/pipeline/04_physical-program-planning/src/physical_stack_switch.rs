//! Select the stack transfers and state effects for the physical entry.
use crate::native_entry_bridge::NativeEntryBridgePlan;
use calling_conventions::{CallPlan, EntryControl, IndirectPointerLocation, ValueLocation};
use diagnostics::Diagnostic;
use physical_recipes::physical_stack_switch as recipes;
use recipes::StackSwitchInstructions;
pub use recipes::{PhysicalIncomingStackRequirement, StackTransfer};
pub type StackSwitchPlan = recipes::StackSwitchPlan<crate::PhysicalRecipeOrigin>;
fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("physical image entry: {reason}"))
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

pub fn plan(bridge: NativeEntryBridgePlan) -> Result<StackSwitchPlan, Diagnostic> {
    let inbound: &CallPlan = &bridge.request().inbound.call;
    let alignment = u32::from(inbound.stack_alignment);
    if inbound.policy.architecture() != target::Architecture::X86_64
        || inbound.entry_control != EntryControl::CallReturn
        || alignment < 16
        || !alignment.is_power_of_two()
    {
        return Err(invalid("private entry stack requires an aligned x64 call"));
    }
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
    let mut incoming_readable_bytes = 8u32;
    for &(offset, byte_count) in &stack_pieces {
        let argument_end = offset
            .checked_add(byte_count)
            .ok_or_else(|| invalid("incoming stack argument extent overflows"))?;
        frame_end = frame_end.max(argument_end);
        incoming_readable_bytes = incoming_readable_bytes.max(
            argument_end
                .checked_add(8)
                .ok_or_else(|| invalid("incoming stack argument return bias overflows"))?,
        );
    }
    let outgoing_frame_bytes = frame_end
        .checked_add(alignment - 1)
        .map(|bytes| bytes & !(alignment - 1))
        .filter(|bytes| *bytes <= i32::MAX as u32 - 8)
        .ok_or_else(|| invalid("private entry call frame exceeds x64 displacement range"))?;
    let mut transfers = Vec::new();
    for (offset, byte_count) in stack_pieces {
        let mut copied = 0;
        while copied < byte_count {
            let width = match byte_count - copied {
                8.. => 8,
                4.. => 4,
                2.. => 2,
                _ => 1,
            };
            let destination = offset + copied;
            transfers.push(StackTransfer {
                source: destination + 8,
                destination,
                width,
            });
            copied += width;
        }
    }
    use calling_conventions::{
        MachineRegister as R, MachineState as S, MachineStateSet, RegisterSet, StateEffectsEvidence,
    };
    let effects = bridge.effects().compose(&StateEffectsEvidence::new(
        RegisterSet::new([R::X86Rsp, R::X86Rax, R::X86R10]),
        RegisterSet::new([R::X86Rsp, R::X86Rax, R::X86R10]),
        MachineStateSet::new([S::StackPointer]),
        MachineStateSet::new([S::StackPointer, S::Flags]),
    ));
    let complete_effects = bridge
        .request()
        .checked_complete_effects(&effects)
        .map_err(|reason| invalid(&reason.to_string()))?;
    Ok(StackSwitchPlan::from_planner(
        crate::PhysicalRecipeOrigin(()),
        StackSwitchInstructions {
            bridge,
            transfers,
            outgoing_frame_bytes,
            incoming: PhysicalIncomingStackRequirement {
                bytes_below_entry_rsp: 0,
                readable_bytes_from_entry_rsp: u64::from(incoming_readable_bytes),
                caller_alignment: u64::from(alignment),
                caller_rsp_bias: 8,
            },
            effects,
            complete_effects,
        },
    ))
}
