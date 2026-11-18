//! Encode the checked physical stack transfer program and relocation fields.
use crate::native_entry_bridge::{NativeEntryBridge, encode_native_entry_bridge};
use arena::Arena;
use diagnostics::Diagnostic;
use physical_recipes::physical_stack_switch::{
    StackSwitchInstructions, StackSwitchPlan, StackTransfer,
};
fn invalid(reason: &str) -> Diagnostic {
    Diagnostic::error(format!("physical image entry: {reason}"))
}

pub use physical_recipes::physical_stack_switch::{Destination, Fixup, StackSwitch};

pub fn encode<Origin>(
    plan: StackSwitchPlan<Origin>,
) -> Result<(NativeEntryBridge<Origin>, StackSwitch), Diagnostic> {
    let StackSwitchInstructions {
        bridge,
        transfers,
        outgoing_frame_bytes,
        incoming,
        effects,
        complete_effects,
    } = plan.into_instructions();
    let bridge = encode_native_entry_bridge(bridge).map_err(|error| invalid(&error.to_string()))?;
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
    for StackTransfer {
        source,
        destination,
        width,
    } in transfers
    {
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
    let body_call = u32::try_from(bridge_offset + bridge.call_field_offset())
        .map_err(|_| invalid("physical entry body call exceeds u32"))?;
    Ok((
        bridge,
        StackSwitch {
            bytes,
            fixups,
            outgoing_frame_bytes,
            incoming,
            effects,
            complete_effects,
            body_call,
        },
    ))
}
