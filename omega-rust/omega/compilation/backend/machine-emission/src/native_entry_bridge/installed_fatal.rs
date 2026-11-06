//! Enter a nonreturning handler with the installation's retained shared context.
//! The CPU frame stays opaque. Recovery follows the installed timer gate's
//! actual IST selector, then the active TSS and its leased root word. The gate's
//! semantic stack class never supplies a hardware selector.

use super::*;
use calling_conventions::{EntryStack, Preemption};
use runtime_abi::static_interrupts::{InstalledInterruptProtocol, root};

#[cfg(test)]
mod tests;

pub(super) fn emit(
    request: NativeEntryBridgeRequest,
    plan: InstalledInterruptEntryPlan,
) -> Result<NativeEntryBridge, Error> {
    let InstalledInterruptProtocol::FatalContext {
        anchor_gate_byte_offset,
        table_bytes,
        ..
    } = plan.protocol
    else {
        return Err(Error::InvalidInstalledInterruptGeometry);
    };
    if !plan.has_canonical_geometry() {
        return Err(Error::InvalidInstalledInterruptGeometry);
    }
    entry_protocol::validate_fatal(&request)?;
    let inbound = &request.inbound.call;
    let internal = &request.internal;
    if request.target.architecture != Architecture::X86_64
        || inbound.entry_control != EntryControl::InterruptReturn
        || internal.entry_control != EntryControl::CallReturn
        || internal.policy.architecture() != Architecture::X86_64
        || inbound.policy.architecture() != Architecture::X86_64
        || internal.stack_alignment != 16
        || !matches!(
            request.inbound.state.stack,
            EntryStack::Interrupted | EntryStack::Dedicated { .. }
        )
        || request.inbound.state.preemption != Preemption::Masked
    {
        return Err(Error::UnsupportedInterruptState);
    }
    if internal.parameters.len() != 1
        || inbound.parameters.len() != 1
        || request.parameters.len() != 1
        || internal.result.is_some()
        || inbound.result.is_some()
        || !internal.callback_materializations.is_empty()
        || !inbound.callback_materializations.is_empty()
    {
        return Err(Error::UnsupportedFatalInterruptSignature);
    }
    let (_, parameter) = request
        .parameters
        .iter()
        .next()
        .ok_or(Error::InvalidParameterJoin)?;
    if parameter.inbound_ordinal != 0
        || parameter.scalar_codec != 0
        || inbound.parameters[0].shape != ValueShape::integer(8, 8)
        || internal.parameters[0].shape.class != ValueClass::BorrowedReference
    {
        return Err(Error::UnsupportedFatalInterruptSignature);
    }
    let transport = value_transport(&inbound.parameters[0], &internal.parameters[0], 0)?;
    if transport.destination.pieces().iter().any(|piece| {
        matches!(piece.location, WordLocation::Register(register) if register.codec_index() >= 16)
    }) {
        return Err(Error::UnsupportedFatalInterruptSignature);
    }
    let mut cursor = u32::from(internal.shadow_bytes);
    for piece in transport.destination.pieces() {
        if let WordLocation::Stack(offset) = piece.location {
            cursor = cursor.max(
                offset
                    .checked_add(u32::from(piece.byte_size))
                    .ok_or(Error::FrameOverflow)?,
            );
        }
    }
    let encoding = Encoding {
        architecture: Architecture::X86_64,
        model: validate_physical_register_model(isa_x86_64::x86_64_physical_register_model())
            .map_err(|_| Error::Encoding)?,
        scratch: MachineRegister::X86Rax,
        address_scratch: MachineRegister::X86Rdx,
        aligned_homes: Vec::new(),
    };
    let idtr = allocate_value(&mut cursor, 16)?;
    let gdtr = allocate_value(&mut cursor, 16)?;
    let provider = allocate_word(&mut cursor)?;
    let ist = allocate_word(&mut cursor)?;
    let context = allocate_word(&mut cursor)?;
    let local_bytes = cursor
        .checked_add(15)
        .map(|value| value & !15)
        .filter(|value| *value <= i32::MAX as u32)
        .ok_or(Error::FrameOverflow)?;
    let interval = crate::frame_layout::stack_commit::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(local_bytes) > interval {
        u32::try_from(u64::from(local_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let (prologue, _) = isa_x86_64::encode_system_v_amd64_frame_protocol(
        &encoding.model,
        u64::from(local_bytes),
        isa_x86_64::X86_64StackProbe {
            interval_bytes: interval,
            touches,
        },
        &[],
    )
    .map_err(|_| Error::Encoding)?;
    // A dedicated IST top is checked to be 16-byte aligned. An interrupted
    // stack can lose another fifteen bytes when the CPU aligns it before
    // constructing the frame, independently of our later AND RSP.
    let cpu_alignment_bytes = match request.inbound.state.stack {
        EntryStack::Interrupted => 15,
        EntryStack::Dedicated { .. } => 0,
        _ => return Err(Error::UnsupportedInterruptState),
    };
    let frame_bytes = plan
        .hardware_frame_bytes()
        .checked_add(cpu_alignment_bytes)
        .and_then(|bytes| bytes.checked_add(plan.saved_register_bytes))
        .and_then(|bytes| bytes.checked_add(15))
        .and_then(|bytes| bytes.checked_add(local_bytes))
        .ok_or(Error::FrameOverflow)?;
    let mut bytes = Vec::new();
    for register in super::installed_interrupt::SAVED_GPRS {
        super::installed_interrupt::push_or_pop(&mut bytes, register, false);
    }
    bytes.extend_from_slice(&isa_x86_64::CLEAR_DIRECTION_FLAG_BYTES);
    // The opaque frame may include an error word. Alignment consumes at most
    // fifteen additional bytes; no return path needs the original stack cursor.
    bytes.extend_from_slice(&[0x48, 0x83, 0xe4, 0xf0]); // AND RSP,-16
    bytes.extend_from_slice(&prologue);
    descriptor_store(&mut bytes, idtr, 1); // SIDT
    descriptor_store(&mut bytes, gdtr, 0); // SGDT
    encoding.memory(&mut bytes, MachineRegister::X86Rax, idtr, 2, true)?;
    compare_eax(&mut bytes, table_bytes - 1);
    require(&mut bytes, 0x74); // exact IDTR limit
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, idtr + 2, 8, true)?;
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 4, 1, true)?;
    bytes.extend_from_slice(&[0x83, 0xe0, 7, 0x85, 0xc0]); // IST = gate.ist & 7
    require(&mut bytes, 0x75);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, ist, 8, false)?;
    // Reassemble the timer's exact installed entry address from its gate.
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 8, 4, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 32, 0x48, 0x89, 0xc1]);
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset + 6, 2, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 16, 0x48, 0x09, 0xc1]);
    encoding.pointer_memory(&mut bytes, anchor_gate_byte_offset, 2, true)?;
    bytes.extend_from_slice(&[0x48, 0x09, 0xc8]);
    encoding.memory(&mut bytes, MachineRegister::X86Rax, provider, 8, false)?;
    // STR selects a GDT TSS descriptor. The installer validates its complete
    // extent, live type and exact backing before publishing this IDT.
    bytes.extend_from_slice(&[0x0f, 0x00, 0xc8, 0x0f, 0xb7, 0xc0]); // STR AX; MOVZX EAX,AX
    bytes.extend_from_slice(&[0xa9, 7, 0, 0, 0]); // TEST EAX,7 (no LDT or RPL)
    require(&mut bytes, 0x74);
    bytes.extend_from_slice(&[0x85, 0xc0]);
    require(&mut bytes, 0x75);
    bytes.extend_from_slice(&[0x48, 0x89, 0xc1, 0x83, 0xc0, 15]);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, gdtr, 2, true)?;
    bytes.extend_from_slice(&[0x39, 0xd0]); // CMP EAX,EDX: last descriptor byte <= GDTR.limit
    require(&mut bytes, 0x76);
    encoding.memory(&mut bytes, MachineRegister::X86Rdx, gdtr + 2, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x01, 0xca]); // ADD RDX,RCX
    encoding.pointer_memory(&mut bytes, 4, 4, true)?;
    bytes.extend_from_slice(&[0x25, 0, 0, 0, 0xff, 0x48, 0x89, 0xc1]);
    encoding.pointer_memory(&mut bytes, 2, 2, true)?;
    bytes.extend_from_slice(&[0x48, 0x09, 0xc1]);
    encoding.pointer_memory(&mut bytes, 4, 1, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 16, 0x48, 0x09, 0xc1]);
    encoding.pointer_memory(&mut bytes, 8, 4, true)?;
    bytes.extend_from_slice(&[0x48, 0xc1, 0xe0, 32, 0x48, 0x09, 0xc8, 0x48, 0x89, 0xc2]);
    encoding.memory(&mut bytes, MachineRegister::X86Rcx, ist, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x8b, 0x54, 0xca, 28]); // MOV RDX,[TSS+IST*8+28]
    bytes.extend_from_slice(&[0x48, 0x8b, 0x52, 0xc8]); // MOV RDX,[IST_top-56]
    encoding.pointer_memory(&mut bytes, root::STATE, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x83, 0xf8, root::ACTIVE as u8]);
    require(&mut bytes, 0x74);
    encoding.pointer_memory(&mut bytes, root::PROVIDER, 8, true)?;
    encoding.memory(&mut bytes, MachineRegister::X86Rcx, provider, 8, true)?;
    bytes.extend_from_slice(&[0x48, 0x39, 0xc8]);
    require(&mut bytes, 0x74);
    encoding.pointer_memory(&mut bytes, root::CONTEXT, 8, true)?;
    encoding.memory(&mut bytes, MachineRegister::X86Rax, context, 8, false)?;
    encoding.distribute_argument(&mut bytes, &transport.destination, context, 8)?;
    let call_field_offset = bytes.len() + 1;
    bytes.extend_from_slice(&isa_x86_64::ORDINARY_DIRECT_CALL_BYTES);
    // Admission proves this body has no normal completion. A violated physical
    // contract must stop here; it must never IRET through an opaque frame.
    halt(&mut bytes);
    finish_native_entry(request, bytes, frame_bytes, call_field_offset, Some(plan))
}

fn descriptor_store(bytes: &mut Vec<u8>, offset: u32, operation: u8) {
    bytes.extend_from_slice(&[0x0f, 0x01, 0x84 | (operation << 3), 0x24]);
    bytes.extend_from_slice(&offset.to_le_bytes());
}

fn compare_eax(bytes: &mut Vec<u8>, value: u32) {
    bytes.push(0x3d);
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn require(bytes: &mut Vec<u8>, condition: u8) {
    bytes.extend_from_slice(&[condition, 4]);
    halt(bytes);
}

fn halt(bytes: &mut Vec<u8>) {
    bytes.extend_from_slice(&isa_x86_64::PRIVILEGED_STOP_BYTES);
}
