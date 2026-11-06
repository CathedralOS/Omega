//! Arrival transport for one explicitly admitted installed timer handler.
//!
//! The installation owns the dedicated stack and its root slot. Unlike ordinary
//! entry transport, interrupted registers are not semantic arguments: this
//! adapter saves them all, then constructs the one owned acknowledgement from
//! the admitted root. These bytes do not establish installation authority.

use calling_conventions::{EntryStack, Preemption};
use runtime_abi::static_interrupts::{acknowledgement as ack, root};

use super::*;

/// Emit a returning installed entry from its retained installation plan. The
/// caller must separately admit the root, stack lease and exact handler contract.
pub fn emit_installed_interrupt_entry_bridge(
    request: NativeEntryBridgeRequest,
    plan: InstalledInterruptEntryPlan,
) -> Result<NativeEntryBridge, NativeEntryBridgeError> {
    if !plan.has_canonical_geometry() {
        return Err(Error::InvalidInstalledInterruptGeometry);
    }
    if matches!(
        plan.protocol,
        runtime_abi::static_interrupts::InstalledInterruptProtocol::FatalContext { .. }
    ) {
        return super::installed_fatal::emit(request, plan);
    }
    let protocol = entry_protocol::select(&request)?;
    let internal = &request.internal;
    let inbound = &request.inbound.call;
    if request.target.architecture != Architecture::X86_64
        || inbound.entry_control != EntryControl::InterruptReturn
        || internal.entry_control != EntryControl::CallReturn
        || internal.policy.architecture() != Architecture::X86_64
        || inbound.policy.architecture() != Architecture::X86_64
        || internal.stack_alignment != 16
        || !matches!(request.inbound.state.stack, EntryStack::Dedicated { .. })
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
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let (_, parameter) = request
        .parameters
        .iter()
        .next()
        .ok_or(Error::InvalidParameterJoin)?;
    if parameter.inbound_ordinal != 0 || parameter.scalar_codec != 0 {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let source = &inbound.parameters[0];
    let destination = &internal.parameters[0];
    if source.shape.byte_size != ack::BYTES
        || u32::from(source.shape.alignment) != runtime_abi::static_interrupts::ALIGNMENT
        || is_borrowed(source.shape.class)
        || is_borrowed(destination.shape.class)
    {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let transport = value_transport(source, destination, 0)?;
    // Only integer lanes may be touched: the entry promise saves no vector state.
    if transport.destination.pieces().iter().any(|piece| {
        matches!(piece.location, WordLocation::Register(register) if register.codec_index() >= 16)
    }) {
        return Err(Error::UnsupportedInstalledInterruptSignature);
    }
    let mut cursor = u32::from(internal.shadow_bytes);
    if let ValueStorage::Indirect {
        copy_offset: Some(offset),
        ..
    } = &transport.destination
    {
        cursor = cursor.max(offset.checked_add(ack::BYTES).ok_or(Error::FrameOverflow)?);
    }
    for piece in transport.destination.pieces() {
        if let WordLocation::Stack(offset) = piece.location {
            cursor = cursor.max(
                offset
                    .checked_add(u32::from(piece.byte_size))
                    .ok_or(Error::FrameOverflow)?,
            );
        }
    }
    let occupied = |register| {
        transport.destination.pieces().iter().any(|piece| {
        matches!(piece.location, WordLocation::Register(candidate) if candidate == register)
    })
    };
    let mut candidates = [
        MachineRegister::X86R11,
        MachineRegister::X86R10,
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
    ]
    .into_iter()
    .filter(|register| !occupied(*register));
    let scratch = candidates.next().ok_or(Error::NoUnoccupiedScratch)?;
    let address_scratch = candidates.next().ok_or(Error::NoUnoccupiedScratch)?;
    let mut encoding = Encoding {
        architecture: Architecture::X86_64,
        model: validate_physical_register_model(isa_x86_64::x86_64_physical_register_model())
            .map_err(|_| Error::Encoding)?,
        scratch,
        address_scratch,
        aligned_homes: Vec::new(),
    };
    let snapshot = allocate_home(&mut cursor, ack::BYTES, 8, &mut encoding.aligned_homes)?;
    // The body owns Ack and may overwrite it. Keep the post-call root separately.
    let saved_root = allocate_word(&mut cursor)?;
    let local_bytes = cursor
        .checked_add(15)
        .map(|value| value & !15)
        .filter(|value| *value <= i32::MAX as u32)
        .ok_or(Error::FrameOverflow)?;
    let frame_bytes = plan
        .root_slot_from_top
        .checked_add(plan.saved_register_bytes)
        .and_then(|value| value.checked_add(local_bytes))
        .ok_or(Error::FrameOverflow)?;
    let root_offset = local_bytes
        .checked_add(plan.saved_register_bytes)
        .ok_or(Error::FrameOverflow)?;
    let interval = crate::frame_layout::stack_commit::stack_commit_granule_bytes(request.target)
        .ok_or(Error::IncompatiblePlan)?;
    let touches = if u64::from(local_bytes) > interval {
        u32::try_from(u64::from(local_bytes).div_ceil(interval))
            .map_err(|_| Error::FrameOverflow)?
    } else {
        0
    };
    let (prologue, epilogue) = isa_x86_64::encode_system_v_amd64_frame_protocol(
        &encoding.model,
        u64::from(local_bytes),
        isa_x86_64::X86_64StackProbe {
            interval_bytes: interval,
            touches,
        },
        &[],
    )
    .map_err(|_| Error::Encoding)?;

    let mut bytes = Vec::new();
    // Normalize both CPU frames to top-56. SUB changes only live flags; the
    // interrupted flags already reside in the untouched hardware return frame.
    let gap = u8::try_from(plan.root_slot_from_top - plan.hardware_frame_bytes())
        .map_err(|_| Error::InvalidInstalledInterruptGeometry)?;
    bytes.extend_from_slice(&[0x48, 0x83, 0xec, gap]); // sub rsp, gap
    for register in SAVED_GPRS {
        push_or_pop(&mut bytes, register, false);
    }
    bytes.extend_from_slice(protocol.before_body); // CLD, after every GPR save.
    bytes.extend_from_slice(&prologue);
    encoding.memory(&mut bytes, address_scratch, root_offset, 8, true)?;
    encoding.memory(&mut bytes, address_scratch, saved_root, 8, false)?;
    encoding.memory(&mut bytes, address_scratch, snapshot + ack::ROOT, 8, false)?;
    encoding.pointer_memory(&mut bytes, root::STATE, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, root::ACTIVE as u8);
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, 0);
    encoding.pointer_memory(&mut bytes, root::INVOCATION, 8, true)?;
    add_one_without_overflow(&mut bytes, scratch);
    encoding.pointer_memory(&mut bytes, root::INVOCATION, 8, false)?;
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, false)?;
    encoding.memory(&mut bytes, scratch, snapshot + ack::INVOCATION, 8, false)?;
    encoding.memory(&mut bytes, scratch, snapshot + ack::DEBT, 8, false)?;
    for (root_field, token_field) in [
        (root::PROVIDER, ack::PROVIDER),
        (root::COMPLETION, ack::COMPLETION),
    ] {
        encoding.pointer_memory(&mut bytes, root_field, 8, true)?;
        encoding.memory(&mut bytes, scratch, snapshot + token_field, 8, false)?;
    }
    encoding.distribute_argument(&mut bytes, &transport.destination, snapshot, ack::BYTES)?;
    let call_field_offset = bytes.len() + 1;
    bytes.extend_from_slice(&isa_x86_64::ORDINARY_DIRECT_CALL_BYTES);
    encoding.memory(&mut bytes, address_scratch, saved_root, 8, true)?;
    encoding.pointer_memory(&mut bytes, root::DEBT, 8, true)?;
    require_scratch_equal(&mut bytes, scratch, 0);
    bytes.extend_from_slice(&epilogue);
    for register in SAVED_GPRS.into_iter().rev() {
        push_or_pop(&mut bytes, register, true);
    }
    // Both paths are back at top-56; discard gap and any CPU error word.
    bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 16]); // add rsp, 16
    bytes.extend_from_slice(protocol.return_bytes);
    Ok(NativeEntryBridge {
        request,
        bytes,
        frame_bytes,
        call_field_offset,
        installed_interrupt: Some(plan),
    })
}

// Encoding order is also restoration order; RSP comes from the CPU frame.
pub(super) const SAVED_GPRS: [u8; 15] = [0, 1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

pub(super) fn push_or_pop(bytes: &mut Vec<u8>, register: u8, pop: bool) {
    if register >= 8 {
        bytes.push(0x41);
    }
    bytes.push(if pop { 0x58 } else { 0x50 } + (register & 7));
}

fn require_scratch_equal(bytes: &mut Vec<u8>, register: MachineRegister, value: u8) {
    let register = register.codec_index();
    bytes.extend_from_slice(&[0x48 | (register >> 3), 0x83, 0xf8 | (register & 7), value]);
    // JE skips the internal invalid-token trap. This is not an authored crash.
    bytes.extend_from_slice(&[0x74, 0x02, 0x0f, 0x0b]);
}

fn add_one_without_overflow(bytes: &mut Vec<u8>, register: MachineRegister) {
    let register = register.codec_index();
    bytes.extend_from_slice(&[0x48 | (register >> 3), 0x83, 0xc0 | (register & 7), 1]);
    // JNC skips UD2. Starting at zero yields one; MAX never wraps into a token.
    bytes.extend_from_slice(&[0x73, 0x02, 0x0f, 0x0b]);
}
