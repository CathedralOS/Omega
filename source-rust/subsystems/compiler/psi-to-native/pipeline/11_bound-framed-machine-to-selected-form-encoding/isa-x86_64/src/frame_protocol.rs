/*
We encode and validate x86-64 fixed-frame stack transport, 4 KiB page-granule
stack-commit probes (`X86_64StackProbe`), callee-saved slot save/restore
(`X86_64FrameSlot`), over-aligned frame home rounding, and pointer memory
accesses across System V AMD64 and Microsoft x64 frames. Each prologue and
epilogue instruction also records its unwind effect (`FrameProtocolStep`), so
image writers can describe the frame without decoding these bytes.
*/

use calling_conventions::MachineRegister;
use calling_conventions::frame_unwind::{
    EncodedFrameProtocol, FrameProtocolEffect, FrameProtocolSteps,
};
use register_model::{RegisterViewId, ValidatedPhysicalRegisterModel};
pub use target_x86_64::frame_protocol::*;

pub fn encode_planned_system_v_amd64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    plan: &X86_64FrameProtocolPlan,
) -> Result<EncodedFrameProtocol, X86_64FrameProtocolError> {
    if model.identity() != plan.model() {
        return Err(X86_64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    let frame_size_bytes = plan.frame_size_bytes();
    let probe = plan.probe();
    let slots = plan.slots();
    let mut prologue = Vec::new();
    let mut epilogue = Vec::new();
    let mut prologue_steps = FrameProtocolSteps::default();
    let mut epilogue_steps = FrameProtocolSteps::default();
    if probe.touches == 0 {
        append_stack_adjust(&mut prologue, &mut prologue_steps, frame_size_bytes, true)?;
    } else {
        let mut remaining = frame_size_bytes;
        loop {
            let chunk = remaining.min(X86_64_STACK_PROBE_INTERVAL_BYTES);
            append_stack_adjust(&mut prologue, &mut prologue_steps, chunk, true)?;
            prologue.extend_from_slice(&[0x80, 0x3c, 0x24, 0x00]);
            record(
                &mut prologue_steps,
                &prologue,
                FrameProtocolEffect::Unchanged,
            )?;
            remaining -= chunk;
            if remaining == 0 {
                break;
            }
        }
    }
    for slot in slots {
        let code = register_code(model, slot.view)?;
        append_register_memory(&mut prologue, code, slot.offset_bytes, false)?;
        record(&mut prologue_steps, &prologue, saved_register(code, slot)?)?;
    }
    for slot in slots.iter().rev() {
        let code = register_code(model, slot.view)?;
        append_register_memory(&mut epilogue, code, slot.offset_bytes, true)?;
        record(&mut epilogue_steps, &epilogue, saved_register(code, slot)?)?;
    }
    append_stack_adjust(&mut epilogue, &mut epilogue_steps, frame_size_bytes, false)?;
    Ok(EncodedFrameProtocol {
        prologue,
        epilogue,
        prologue_steps: prologue_steps.into_steps(),
        epilogue_steps: epilogue_steps.into_steps(),
    })
}

pub fn encode_system_v_amd64_frame_protocol(
    model: &ValidatedPhysicalRegisterModel,
    frame_size_bytes: u64,
    probe: X86_64StackProbe,
    slots: &[X86_64FrameSlot],
) -> Result<EncodedFrameProtocol, X86_64FrameProtocolError> {
    encode_planned_system_v_amd64_frame_protocol(
        model,
        &plan_system_v_amd64_frame_protocol(model, frame_size_bytes, probe, slots)?,
    )
}

fn record(
    steps: &mut FrameProtocolSteps,
    bytes: &[u8],
    effect: FrameProtocolEffect,
) -> Result<(), X86_64FrameProtocolError> {
    steps
        .push(bytes, effect)
        .ok_or(X86_64FrameProtocolError::InvalidFrameSize)
}

fn saved_register(
    code: u8,
    slot: &X86_64FrameSlot,
) -> Result<FrameProtocolEffect, X86_64FrameProtocolError> {
    const REGISTERS: [MachineRegister; 16] = [
        MachineRegister::X86Rax,
        MachineRegister::X86Rcx,
        MachineRegister::X86Rdx,
        MachineRegister::X86Rbx,
        MachineRegister::X86Rsp,
        MachineRegister::X86Rbp,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R8,
        MachineRegister::X86R9,
        MachineRegister::X86R10,
        MachineRegister::X86R11,
        MachineRegister::X86R12,
        MachineRegister::X86R13,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ];
    Ok(FrameProtocolEffect::Register {
        register: REGISTERS[usize::from(code & 15)],
        frame_offset_bytes: u32::try_from(slot.offset_bytes)
            .map_err(|_| X86_64FrameProtocolError::OffsetOutOfRange)?,
    })
}

fn register_code(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
) -> Result<u8, X86_64FrameProtocolError> {
    let name = model
        .model()
        .view(view)
        .map(|row| row.name.as_str())
        .ok_or(X86_64FrameProtocolError::UnknownOrUnsupportedView(view))?;
    let code = match name {
        "rax" => 0,
        "rcx" => 1,
        "rdx" => 2,
        "rbx" => 3,
        "rbp" => 5,
        "rsi" => 6,
        "rdi" => 7,
        "r8" => 8,
        "r9" => 9,
        "r10" => 10,
        "r11" => 11,
        "r12" => 12,
        "r13" => 13,
        "r14" => 14,
        "r15" => 15,
        _ => return Err(X86_64FrameProtocolError::UnknownOrUnsupportedView(view)),
    };
    Ok(code)
}

fn append_stack_adjust(
    bytes: &mut Vec<u8>,
    steps: &mut FrameProtocolSteps,
    amount: u64,
    subtract: bool,
) -> Result<(), X86_64FrameProtocolError> {
    if amount == 0 {
        return Ok(());
    }
    let operation = if subtract { 0xec } else { 0xc4 };
    let amount = i32::try_from(amount).map_err(|_| X86_64FrameProtocolError::InvalidFrameSize)?;
    if amount <= 127 {
        bytes.extend_from_slice(&[0x48, 0x83, operation, amount as u8]);
    } else {
        bytes.extend_from_slice(&[0x48, 0x81, operation]);
        crate::codec_bytes::push_encoded(bytes, amount.to_le_bytes());
    }
    record(
        steps,
        bytes,
        FrameProtocolEffect::AdjustStack {
            bytes: amount as u32,
        },
    )
}

fn append_register_memory(
    bytes: &mut Vec<u8>,
    register: u8,
    offset: u64,
    load: bool,
) -> Result<(), X86_64FrameProtocolError> {
    let rex = 0x48 | ((register >> 3) << 2);
    bytes.push(rex);
    bytes.push(if load { 0x8b } else { 0x89 });
    let low = register & 7;
    if offset == 0 {
        bytes.push((low << 3) | 0x04);
        bytes.push(0x24);
    } else if offset <= 127 {
        bytes.push(0x40 | (low << 3) | 0x04);
        bytes.push(0x24);
        bytes.push(offset as u8);
    } else {
        let offset =
            u32::try_from(offset).map_err(|_| X86_64FrameProtocolError::OffsetOutOfRange)?;
        if offset > i32::MAX as u32 {
            return Err(X86_64FrameProtocolError::OffsetOutOfRange);
        }
        bytes.push(0x80 | (low << 3) | 0x04);
        bytes.push(0x24);
        crate::codec_bytes::push_encoded(bytes, offset.to_le_bytes());
    }
    Ok(())
}

pub fn encode_x86_64_frame_memory(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    offset_bytes: u32,
    byte_size: u8,
    load: bool,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    encode_memory(model, view, 4, offset_bytes, byte_size, load)
}

pub fn encode_x86_64_pointer_memory(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    base: RegisterViewId,
    offset_bytes: u32,
    byte_size: u8,
    load: bool,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    encode_memory(
        model,
        view,
        register_code(model, base)?,
        offset_bytes,
        byte_size,
        load,
    )
}

fn encode_memory(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    base: u8,
    offset_bytes: u32,
    byte_size: u8,
    load: bool,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    if model.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    let name = model
        .model()
        .view(view)
        .ok_or(X86_64FrameProtocolError::UnknownOrUnsupportedView(view))?
        .name
        .as_str();
    let mut bytes = Vec::new();
    if let Some(number) = name.strip_prefix("xmm") {
        let number = number
            .parse::<u8>()
            .ok()
            .filter(|number| *number < 16)
            .ok_or(X86_64FrameProtocolError::UnknownOrUnsupportedView(view))?;
        let prefix = match byte_size {
            4 => Some(0xf3),
            8 => Some(0xf2),
            16 => None,
            _ => return Err(X86_64FrameProtocolError::UnsupportedTransferWidth(byte_size)),
        };
        if let Some(prefix) = prefix {
            bytes.push(prefix);
        }
        let rex = 0x40 | ((number >> 3) << 2) | (base >> 3);
        if rex != 0x40 {
            bytes.push(rex);
        }
        bytes.extend_from_slice(&[0x0f, if load { 0x10 } else { 0x11 }]);
        append_memory_operand(&mut bytes, number, base, offset_bytes)?;
        return Ok(bytes);
    }
    let register = register_code(model, view)?;
    if byte_size == 8 && base == 4 {
        append_register_memory(&mut bytes, register, u64::from(offset_bytes), load)?;
        return Ok(bytes);
    }
    if !matches!(byte_size, 1 | 2 | 4 | 8) {
        return Err(X86_64FrameProtocolError::UnsupportedTransferWidth(byte_size));
    }
    if byte_size == 2 && !load {
        bytes.push(0x66);
    }
    let rex = 0x40 | ((register >> 3) << 2) | (base >> 3) | if byte_size == 8 { 8 } else { 0 };
    if rex != 0x40 || (byte_size == 1 && !load && register >= 4) {
        bytes.push(rex);
    }
    match (load, byte_size) {
        (true, 1) => bytes.extend_from_slice(&[0x0f, 0xb6]),
        (true, 2) => bytes.extend_from_slice(&[0x0f, 0xb7]),
        (true, 4 | 8) => bytes.push(0x8b),
        (false, 1) => bytes.push(0x88),
        (false, _) => bytes.push(0x89),
        _ => return Err(X86_64FrameProtocolError::UnsupportedTransferWidth(byte_size)),
    }
    append_memory_operand(&mut bytes, register, base, offset_bytes)?;
    Ok(bytes)
}

fn append_memory_operand(
    bytes: &mut Vec<u8>,
    register: u8,
    base: u8,
    offset_bytes: u32,
) -> Result<(), X86_64FrameProtocolError> {
    if offset_bytes > i32::MAX as u32 {
        return Err(X86_64FrameProtocolError::OffsetOutOfRange);
    }
    bytes.push(0x80 | ((register & 7) << 3) | (base & 7));
    if base & 7 == 4 {
        bytes.push(0x24);
    }
    bytes.extend_from_slice(&offset_bytes.to_le_bytes());
    Ok(())
}

pub fn encode_x86_64_frame_address(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    offset_bytes: u32,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    let displacement =
        i32::try_from(offset_bytes).map_err(|_| X86_64FrameProtocolError::OffsetOutOfRange)?;
    encode_frame_address_signed(model, view, displacement)
}

pub(crate) fn encode_frame_address_signed(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    displacement: i32,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    if model.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    let register = register_code(model, view)?;
    let mut bytes = vec![0x48 | ((register >> 3) << 2), 0x8d];
    bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
    bytes.extend_from_slice(&displacement.to_le_bytes());
    Ok(bytes)
}

pub fn encode_x86_64_aligned_frame_address(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    offset_bytes: u32,
    alignment_bytes: u16,
    member_offset_bytes: u32,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    if alignment_bytes <= 16
        || !alignment_bytes.is_power_of_two()
        || !offset_bytes.is_multiple_of(16)
    {
        return Err(X86_64FrameProtocolError::NonCanonicalSlots);
    }
    let rounded_offset = offset_bytes
        .checked_add(u32::from(alignment_bytes - 1))
        .ok_or(X86_64FrameProtocolError::OffsetOutOfRange)?;
    let mut bytes = encode_x86_64_frame_address(model, view, rounded_offset)?;
    let register = register_code(model, view)?;
    bytes.extend_from_slice(&[0x48 | (register >> 3), 0x81, 0xe0 | (register & 7)]);
    bytes.extend_from_slice(&(!u32::from(alignment_bytes - 1)).to_le_bytes());
    if member_offset_bytes != 0 {
        bytes.extend_from_slice(&[0x48 | ((register >> 3) << 2) | (register >> 3), 0x8d]);
        append_memory_operand(&mut bytes, register, register, member_offset_bytes)?;
    }
    Ok(bytes)
}

pub(crate) fn validate_frame_address(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    displacement: i32,
    alignment_bytes: u16,
    member_offset_bytes: u32,
    bytes: &[u8],
) -> bool {
    if model.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return false;
    }
    let Ok(register) = register_code(model, view) else {
        return false;
    };
    if alignment_bytes == 1 {
        return decode_address_step(bytes, register, 4) == Some(displacement);
    }
    if alignment_bytes <= 16
        || !alignment_bytes.is_power_of_two()
        || displacement < 0
        || !(displacement as u32).is_multiple_of(16)
        || bytes.len() < 15
    {
        return false;
    }
    let Some(rounded_offset) = displacement.checked_add(i32::from(alignment_bytes - 1)) else {
        return false;
    };
    if decode_address_step(&bytes[..8], register, 4) != Some(rounded_offset)
        || bytes[8..11] != [0x48 | (register >> 3), 0x81, 0xe0 | (register & 7)]
        || u32::from_le_bytes([bytes[11], bytes[12], bytes[13], bytes[14]])
            != !u32::from(alignment_bytes - 1)
    {
        return false;
    }
    if member_offset_bytes == 0 {
        bytes.len() == 15
    } else {
        i32::try_from(member_offset_bytes)
            .ok()
            .is_some_and(|member| {
                decode_address_step(&bytes[15..], register, register) == Some(member)
            })
    }
}

fn decode_address_step(bytes: &[u8], register: u8, base: u8) -> Option<i32> {
    let expected_length = if base & 7 == 4 { 8 } else { 7 };
    if bytes.len() != expected_length
        || bytes[0] != 0x48 | ((register >> 3) << 2) | (base >> 3)
        || bytes[1] != 0x8d
        || bytes[2] != 0x80 | ((register & 7) << 3) | (base & 7)
        || (base & 7 == 4 && bytes[3] != 0x24)
    {
        return None;
    }
    Some(i32::from_le_bytes(
        bytes[expected_length - 4..].try_into().ok()?,
    ))
}

pub(crate) fn decode_pointer_store_suffix<'bytes>(
    model: &ValidatedPhysicalRegisterModel,
    value: RegisterViewId,
    base: RegisterViewId,
    bytes: &'bytes [u8],
) -> Option<&'bytes [u8]> {
    let value = register_code(model, value).ok()?;
    let base = register_code(model, base).ok()?;
    let store_length = if base & 7 == 4 { 8 } else { 7 };
    let prefix_length = bytes.len().checked_sub(store_length)?;
    let store = &bytes[prefix_length..];
    if store[0] != 0x48 | ((value >> 3) << 2) | (base >> 3)
        || store[1] != 0x89
        || store[2] != 0x80 | ((value & 7) << 3) | (base & 7)
        || (base & 7 == 4 && store[3] != 0x24)
        || store[store_length - 4..] != [0, 0, 0, 0]
    {
        return None;
    }
    Some(&bytes[..prefix_length])
}

pub fn encode_x86_64_integer_constant(
    model: &ValidatedPhysicalRegisterModel,
    view: RegisterViewId,
    value: u64,
) -> Result<Vec<u8>, X86_64FrameProtocolError> {
    if model.identity() != crate::canonical_x86_64_physical_register_model_identity() {
        return Err(X86_64FrameProtocolError::PhysicalRegisterModelMismatch);
    }
    let register = register_code(model, view)?;
    let mut bytes = vec![0x48 | (register >> 3), 0xb8 | (register & 7)];
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        X86_64_STACK_PROBE_INTERVAL_BYTES, X86_64FrameProtocolError, X86_64FrameSlot,
        X86_64StackProbe, encode_system_v_amd64_frame_protocol,
    };
    use calling_conventions::MachineRegister;
    use calling_conventions::frame_unwind::{
        EncodedFrameProtocol, FrameProtocolEffect, FrameProtocolStep,
    };
    use register_model::validate_physical_register_model;

    const NO_PROBE: X86_64StackProbe = X86_64StackProbe {
        interval_bytes: X86_64_STACK_PROBE_INTERVAL_BYTES,
        touches: 0,
    };

    #[test]
    fn full_vector_frame_transfer_uses_unaligned_128_bit_moves() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let vector = model.model().view_named("xmm15").unwrap().id;
        for (load, opcode) in [(false, 0x11), (true, 0x10)] {
            assert_eq!(
                super::encode_x86_64_frame_memory(&model, vector, 8, 16, load).unwrap(),
                vec![0x44, 0x0f, opcode, 0xbc, 0x24, 8, 0, 0, 0]
            );
        }
        let integer = model.model().view_named("r15").unwrap().id;
        assert!(super::encode_x86_64_frame_memory(&model, integer, 8, 16, false).is_err());
    }

    #[test]
    fn frame_transfer_reports_the_unsupported_width_for_each_register_class() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        for name in ["rsi", "xmm8"] {
            let view = model.model().view_named(name).unwrap().id;
            assert_eq!(
                super::encode_x86_64_frame_memory(&model, view, 0, 3, true),
                Err(super::X86_64FrameProtocolError::UnsupportedTransferWidth(3))
            );
        }
    }

    #[test]
    fn physical_frame_transfers_keep_byte_and_float_store_extents() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        for (name, offset, width, load, expected) in [
            ("rsi", 3, 1, false, vec![0x40, 0x88, 0xb4, 0x24, 3, 0, 0, 0]),
            (
                "r9",
                2,
                2,
                true,
                vec![0x44, 0x0f, 0xb7, 0x8c, 0x24, 2, 0, 0, 0],
            ),
            (
                "xmm8",
                4,
                4,
                false,
                vec![0xf3, 0x44, 0x0f, 0x11, 0x84, 0x24, 4, 0, 0, 0],
            ),
        ] {
            let view = model.model().view_named(name).unwrap().id;
            assert_eq!(
                super::encode_x86_64_frame_memory(&model, view, offset, width, load).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn pointer_transfers_encode_extended_base_and_exact_tail_width() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        for (value, base, offset, width, load, expected) in [
            (
                "r11",
                "r10",
                24,
                8,
                false,
                vec![0x4d, 0x89, 0x9a, 24, 0, 0, 0],
            ),
            (
                "r9",
                "r12",
                13,
                1,
                false,
                vec![0x45, 0x88, 0x8c, 0x24, 13, 0, 0, 0],
            ),
            (
                "xmm0",
                "r12",
                4,
                8,
                true,
                vec![0xf2, 0x41, 0x0f, 0x10, 0x84, 0x24, 4, 0, 0, 0],
            ),
        ] {
            let value = model.model().view_named(value).unwrap().id;
            let base = model.model().view_named(base).unwrap().id;
            assert_eq!(
                super::encode_x86_64_pointer_memory(&model, value, base, offset, width, load)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn canonical_frame_encodes_adjust_save_restore_and_inverse_adjust() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let rbx = model.model().view_named("rbx").unwrap().id;
        let r12 = model.model().view_named("r12").unwrap().id;
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_system_v_amd64_frame_protocol(
            &model,
            24,
            NO_PROBE,
            &[
                X86_64FrameSlot {
                    view: rbx,
                    offset_bytes: 0,
                    size_bytes: 8,
                },
                X86_64FrameSlot {
                    view: r12,
                    offset_bytes: 8,
                    size_bytes: 8,
                },
            ],
        )
        .unwrap();
        assert_eq!(&prologue[..4], &[0x48, 0x83, 0xec, 24]);
        assert_eq!(&epilogue[epilogue.len() - 4..], &[0x48, 0x83, 0xc4, 24]);
        assert_ne!(prologue, epilogue);
    }

    #[test]
    fn frame_release_never_sign_extends_past_i32() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let probe = |frame_size: u64| X86_64StackProbe {
            interval_bytes: X86_64_STACK_PROBE_INTERVAL_BYTES,
            touches: u32::try_from(frame_size.div_ceil(X86_64_STACK_PROBE_INTERVAL_BYTES)).unwrap(),
        };
        let EncodedFrameProtocol { epilogue, .. } =
            encode_system_v_amd64_frame_protocol(&model, 0x7fff_f000, probe(0x7fff_f000), &[])
                .unwrap();
        assert_eq!(epilogue, [0x48, 0x81, 0xc4, 0x00, 0xf0, 0xff, 0x7f]);
        assert_eq!(
            encode_system_v_amd64_frame_protocol(&model, 0x8000_0000, probe(0x8000_0000), &[]),
            Err(X86_64FrameProtocolError::InvalidFrameSize)
        );
    }

    #[test]
    fn out_of_frame_slot_rejects() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let rbx = model.model().view_named("rbx").unwrap().id;
        assert_eq!(
            encode_system_v_amd64_frame_protocol(
                &model,
                8,
                NO_PROBE,
                &[X86_64FrameSlot {
                    view: rbx,
                    offset_bytes: 8,
                    size_bytes: 8,
                }],
            ),
            Err(X86_64FrameProtocolError::NonCanonicalSlots)
        );
    }

    #[test]
    fn probed_frame_commits_one_granule_per_touch_then_saves() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let rbx = model.model().view_named("rbx").unwrap().id;
        let probe = X86_64StackProbe {
            interval_bytes: X86_64_STACK_PROBE_INTERVAL_BYTES,
            touches: 2,
        };
        let EncodedFrameProtocol {
            prologue, epilogue, ..
        } = encode_system_v_amd64_frame_protocol(
            &model,
            8_192,
            probe,
            &[X86_64FrameSlot {
                view: rbx,
                offset_bytes: 0,
                size_bytes: 8,
            }],
        )
        .unwrap();
        let chunk = [
            0x48, 0x81, 0xec, 0x00, 0x10, 0x00, 0x00, 0x80, 0x3c, 0x24, 0x00,
        ];
        assert_eq!(&prologue[..11], &chunk);
        assert_eq!(&prologue[11..22], &chunk);
        assert_eq!(&prologue[22..26], &[0x48, 0x89, 0x1c, 0x24]);
        assert_eq!(prologue.len(), 26);
        assert_eq!(
            &epilogue[epilogue.len() - 7..],
            &[0x48, 0x81, 0xc4, 0x00, 0x20, 0x00, 0x00]
        );
    }

    #[test]
    fn steps_name_each_instruction_and_what_it_does_to_the_frame() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let rbx = model.model().view_named("rbx").unwrap().id;
        let r12 = model.model().view_named("r12").unwrap().id;
        let probe = X86_64StackProbe {
            interval_bytes: X86_64_STACK_PROBE_INTERVAL_BYTES,
            touches: 2,
        };
        let slot = |view, offset_bytes| X86_64FrameSlot {
            view,
            offset_bytes,
            size_bytes: 8,
        };
        let encoded = encode_system_v_amd64_frame_protocol(
            &model,
            8_192,
            probe,
            &[slot(rbx, 0), slot(r12, 8)],
        )
        .unwrap();
        let step = |end_offset, effect| FrameProtocolStep { end_offset, effect };
        let save = |register, frame_offset_bytes| FrameProtocolEffect::Register {
            register,
            frame_offset_bytes,
        };
        let page = FrameProtocolEffect::AdjustStack { bytes: 4_096 };
        assert_eq!(
            encoded.prologue_steps,
            [
                step(7, page),
                step(11, FrameProtocolEffect::Unchanged),
                step(18, page),
                step(22, FrameProtocolEffect::Unchanged),
                step(26, save(MachineRegister::X86Rbx, 0)),
                step(31, save(MachineRegister::X86R12, 8)),
            ]
        );
        assert_eq!(encoded.prologue.len(), 31);
        assert_eq!(
            encoded.epilogue_steps,
            [
                step(5, save(MachineRegister::X86R12, 8)),
                step(9, save(MachineRegister::X86Rbx, 0)),
                step(16, FrameProtocolEffect::AdjustStack { bytes: 8_192 }),
            ]
        );
        assert_eq!(encoded.epilogue.len(), 16);
    }

    #[test]
    fn probed_frame_touches_the_partial_tail_chunk() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let probe = X86_64StackProbe {
            interval_bytes: X86_64_STACK_PROBE_INTERVAL_BYTES,
            touches: 2,
        };
        let EncodedFrameProtocol { prologue, .. } =
            encode_system_v_amd64_frame_protocol(&model, 4_097, probe, &[]).unwrap();
        assert_eq!(
            &prologue,
            &[
                0x48, 0x81, 0xec, 0x00, 0x10, 0x00, 0x00, 0x80, 0x3c, 0x24, 0x00, 0x48, 0x83, 0xec,
                0x01, 0x80, 0x3c, 0x24, 0x00,
            ]
        );
    }

    #[test]
    fn frame_inside_one_granule_emits_no_probe() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        let EncodedFrameProtocol { prologue, .. } =
            encode_system_v_amd64_frame_protocol(&model, 4_096, NO_PROBE, &[]).unwrap();
        assert_eq!(&prologue, &[0x48, 0x81, 0xec, 0x00, 0x10, 0x00, 0x00]);
    }

    #[test]
    fn probe_roster_must_match_the_committed_frame() {
        let model =
            validate_physical_register_model(crate::x86_64_physical_register_model()).unwrap();
        for (frame_size, touches) in [(8_192_u64, 0), (8_192, 1), (8_192, 3), (4_096, 1), (24, 1)] {
            let probe = X86_64StackProbe {
                interval_bytes: X86_64_STACK_PROBE_INTERVAL_BYTES,
                touches,
            };
            assert_eq!(
                encode_system_v_amd64_frame_protocol(&model, frame_size, probe, &[]),
                Err(X86_64FrameProtocolError::NonCanonicalProbe),
                "size {frame_size} touches {touches}"
            );
        }
        let foreign_interval = X86_64StackProbe {
            interval_bytes: 8_192,
            touches: 1,
        };
        assert_eq!(
            encode_system_v_amd64_frame_protocol(&model, 8_192, foreign_interval, &[]),
            Err(X86_64FrameProtocolError::NonCanonicalProbe)
        );
    }
}
