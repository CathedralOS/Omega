//! Read final instruction bytes and check the selected operand contract.
use ::register_model::{RegisterOperandAccess, RegisterViewId, ValidatedPhysicalRegisterModel};
use selected_instructions::{
    MachineAlternativeKey, MachineEncodedEffects, SelectedInstructionKind,
    SelectedNormalizedForeignCall,
};
use target::NativeTarget;

fn decode_darwin_read_byte(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != crate::hosted_kernel::DARWIN_READ_BYTE_LEAF_BYTES {
        return None;
    }
    let word = |start: usize| -> Option<u32> { ::wire::codec_bytes::u32_at(bytes, start) };
    let home = word(5)?;
    let payload = home.checked_add(4)?;
    if bytes[..5] != [0x31, 0xc0, 0x89, 0x84, 0x24]
        || bytes[9..12] != [0x89, 0x84, 0x24]
        || word(12)? != payload
        || bytes[16..22] != [0x31, 0xff, 0x48, 0x8d, 0xb4, 0x24]
        || word(22)? != payload
        || bytes[26..55]
            != [
                0xba, 1, 0, 0, 0, 0xb8, 3, 0, 0, 2, 0x0f, 0x05, 0x72, 25, 0x48, 0x83, 0xf8, 0,
                0x74, 21, 0x48, 0x83, 0xf8, 1, 0x75, 13, 0xc7, 0x84, 0x24,
            ]
        || word(55)? != home
        || bytes[59..] != [1, 0, 0, 0, 0xeb, 2, 0x0f, 0x0b]
    {
        return None;
    }
    Some(home)
}

fn decode_linux_read_byte(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 62 {
        return None;
    }
    let home = ::wire::codec_bytes::u32_at(bytes, 5)?;
    let payload = home.checked_add(4)?;
    if bytes[..5] != [0x31, 0xc0, 0x89, 0x84, 0x24]
        || bytes[9..12] != [0x89, 0x84, 0x24]
        || ::wire::codec_bytes::u32_at(bytes, 12)? != payload
        || bytes[16..22] != [0x31, 0xff, 0x48, 0x8d, 0xb4, 0x24]
        || ::wire::codec_bytes::u32_at(bytes, 22)? != payload
        || bytes[26..50]
            != [
                0xba, 1, 0, 0, 0, 0x31, 0xc0, 0x0f, 0x05, 0x48, 0x83, 0xf8, 0, 0x74, 21, 0x48,
                0x83, 0xf8, 1, 0x75, 13, 0xc7, 0x84, 0x24,
            ]
        || ::wire::codec_bytes::u32_at(bytes, 50)? != home
        || bytes[54..] != [1, 0, 0, 0, 0xeb, 2, 0x0f, 0x0b]
    {
        return None;
    }
    Some(home)
}

pub fn decode_x86_64_selected_hosted_read_byte(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
    decode_hosted_read_byte_operand(target, bytes)
        .filter(|home| home.is_multiple_of(4) && *home <= 2_147_483_640)
}

/// Return the encoded result home. The tag, payload, branch targets and kernel
/// service must describe the same complete read operation.
pub fn decode_hosted_read_byte_operand(target: NativeTarget, bytes: &[u8]) -> Option<u32> {
    if target == NativeTarget::macos_x64() {
        return decode_darwin_read_byte(bytes);
    }
    if target != NativeTarget::linux_x64() {
        return None;
    }
    decode_linux_read_byte(bytes)
}

#[allow(clippy::too_many_arguments)]

pub fn selected_system_call_effects(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    kind: SelectedInstructionKind,
    alternative: MachineAlternativeKey,
    operand_views: &[RegisterViewId],
    operand_accesses: &[RegisterOperandAccess],
    number: i64,
    bytes: &[u8],
) -> Option<MachineEncodedEffects> {
    if target != NativeTarget::linux_x64()
        || number < 0
        || physical.identity() != crate::canonical_x86_64_physical_register_model_identity()
        || !matches!(kind, SelectedInstructionKind::NormalizedForeignCall { .. })
        || alternative != kind.call_alternative_key()
    {
        return None;
    }
    if bytes.len() != 12 || bytes[..2] != [0x48, 0xb8] || bytes[10..] != [0x0f, 0x05] {
        return None;
    }
    let immediate: [u8; 8] = bytes[2..10].try_into().ok()?;
    if i64::from_le_bytes(immediate) != number {
        return None;
    }
    let catalog = crate::x86_64_register_constraint_catalog_for(physical);
    let keys = crate::x86_64_linux_system_call_keys();
    let row = catalog.row_matching(&keys, operand_views, operand_accesses)?;
    Some(MachineEncodedEffects::supervisor_call_v1(row))
}

pub fn selected_table_call_effects(
    record: &SelectedNormalizedForeignCall,
    alternative: MachineAlternativeKey,
    views: &[RegisterViewId],
    accesses: &[RegisterOperandAccess],
    bytes: &[u8],
) -> Option<MachineEncodedEffects> {
    let (register, offset) = crate::form_effects::table_call::table_address(record)?;
    if bytes.len() != 10
        || bytes[0] != 0x4c | (register >> 3)
        || bytes[1] != 0x8b
        || bytes[2] != 0x98 | (register & 7)
        || bytes[7..] != [0x41, 0xff, 0xd3]
        || u32::from_le_bytes(bytes[3..7].try_into().ok()?) != offset
    {
        return None;
    }
    let row = crate::machine_effects::normalized_foreign_constraint(record)?;
    if views.len() != row.operands.len()
        || accesses.len() != row.operands.len()
        || row
            .operands
            .iter()
            .zip(views.iter().zip(accesses))
            .any(|(operand, (view, access))| {
                operand.fixed_view != Some(*view) || operand.access != *access
            })
    {
        return None;
    }
    let declaration = crate::x86_64_normalized_foreign_call_declaration(record, &row)?;
    let [form] = declaration.alternatives.as_slice() else {
        return None;
    };
    if form.key != alternative {
        return None;
    }
    Some(form.encoded.clone())
}

/// Read the exact port/value load and fixed DX/AL transfers.
pub fn decode_immediate_port_write(bytes: &[u8]) -> Option<(u16, u8)> {
    let (port, tail) = port_operand(bytes)?;
    let &[
        0x49,
        0xbb,
        value,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0x44,
        0x89,
        0xd8,
        0xee,
    ] = tail
    else {
        return None;
    };
    Some((port, value))
}

pub const IMMEDIATE_PORT_READ_U8_WIDTH: usize = 16;

pub fn decode_immediate_port_read_u8(bytes: &[u8]) -> Option<u16> {
    let (port, tail) = port_operand(bytes)?;
    (tail == [0x31, 0xc0, 0xec]).then_some(port)
}

fn port_operand(bytes: &[u8]) -> Option<(u16, &[u8])> {
    let [
        0x49,
        0xba,
        low,
        high,
        0,
        0,
        0,
        0,
        0,
        0,
        0x44,
        0x89,
        0xd2,
        tail @ ..,
    ] = bytes
    else {
        return None;
    };
    Some((u16::from_le_bytes([*low, *high]), tail))
}

/// Read the canonical RSP-relative STMXCSR or LDMXCSR operand. An enlarged
/// displacement form cannot stand in for the shorter original instruction.
fn mxcsr_stack_slot(bytes: &[u8], register_field: u8) -> Option<u32> {
    let [0x0f, 0xae, mod_rm, 0x24, displacement @ ..] = bytes else {
        return None;
    };
    if mod_rm & 0x3f != (register_field << 3) | 4 {
        return None;
    }
    match (mod_rm >> 6, displacement) {
        (0, []) => Some(0),
        (1, [byte @ 1..=127]) => Some(u32::from(*byte)),
        (2, [a, b, c, d]) => {
            let slot = u32::from_le_bytes([*a, *b, *c, *d]);
            (128..=i32::MAX as u32).contains(&slot).then_some(slot)
        }
        _ => None,
    }
}

pub fn floating_control_slots(save: &[u8], restore: &[u8]) -> Option<(u32, u32)> {
    Some((mxcsr_stack_slot(save, 3)?, mxcsr_stack_slot(restore, 2)?))
}

pub fn decode_hosted_exit_process_register(target: NativeTarget, bytes: &[u8]) -> Option<u8> {
    let syscall = crate::hosted_kernel::hosted_exit_syscall_number(target)?;
    if bytes.len() != 12 {
        return None;
    }
    let register = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
    if bytes[0] & 0xfb != 0x40
        || bytes[1] != 0x89
        || bytes[2] & 0xc7 != 0xc7
        || register == 4
        || bytes[3] != 0xb8
        || bytes[4..8] != syscall.to_le_bytes()
        || bytes[8..] != [0x0f, 0x05, 0x0f, 0x0b]
    {
        return None;
    }
    Some(register)
}

pub fn decode_x86_64_selected_hosted_exit_process_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<calling_conventions::MachineRegister> {
    let register = decode_hosted_exit_process_register(target, bytes)?;
    use calling_conventions::MachineRegister::*;
    let registers = [
        X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
        X86R11, X86R12, X86R13, X86R14, X86R15,
    ];
    Some(registers[usize::from(register)])
}

pub fn decode_hosted_write_byte_register(target: NativeTarget, bytes: &[u8]) -> Option<(u8, u32)> {
    let tail: &[u8] = if target == NativeTarget::linux_x64() {
        &[
            0xba, 1, 0, 0, 0, 0xb8, 1, 0, 0, 0, 0x0f, 0x05, 0x48, 0x85, 0xc0, 0x7f, 0x02, 0x0f,
            0x0b,
        ]
    } else if target == NativeTarget::macos_x64() {
        &[
            0xba, 1, 0, 0, 0, 0xb8, 4, 0, 0, 2, 0x0f, 0x05, 0x72, 0x05, 0x48, 0x85, 0xc0, 0x7f,
            0x02, 0x0f, 0x0b,
        ]
    } else {
        return None;
    };
    if bytes.len() != 21 + tail.len() {
        return None;
    }
    let stored_offset = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let addressed_offset = u32::from_le_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]);
    let decoded_register = ((bytes[2] >> 3) & 7) | ((bytes[0] & 4) << 1);
    if bytes[0] & 0xfb != 0x40
        || bytes[1] != 0x88
        || bytes[2] & 0xc7 != 0x84
        || bytes[3] != 0x24
        || decoded_register == 4
        || stored_offset > i32::MAX as u32
        || addressed_offset != stored_offset
        || bytes[8..17] != [0xbf, 1, 0, 0, 0, 0x48, 0x8d, 0xb4, 0x24]
        || bytes[21..] != *tail
    {
        return None;
    }
    Some((decoded_register, stored_offset))
}

pub fn decode_x86_64_selected_hosted_write_byte_i32(
    target: NativeTarget,
    bytes: &[u8],
) -> Option<(calling_conventions::MachineRegister, u32)> {
    let (register, displacement) = decode_hosted_write_byte_register(target, bytes)?;
    use calling_conventions::MachineRegister::*;
    let registers = [
        X86Rax, X86Rcx, X86Rdx, X86Rbx, X86Rsp, X86Rbp, X86Rsi, X86Rdi, X86R8, X86R9, X86R10,
        X86R11, X86R12, X86R13, X86R14, X86R15,
    ];
    Some((registers[usize::from(register)], displacement))
}

#[cfg(test)]
mod tests {
    use super::floating_control_slots;

    #[test]
    fn floating_control_replay_checks_both_directions_and_the_actual_frame_slot() {
        use isa_x86_64::{
            encode_ldmxcsr_rsp_displacement as restore, encode_stmxcsr_rsp_displacement as save,
        };
        let slots = [0, 8, 120, 128, 4096, 2_147_483_640];
        for slot in slots {
            let saved = save(slot).unwrap();
            let restored = restore(slot).unwrap();
            assert_eq!(
                floating_control_slots(&saved, &restored),
                Some((slot, slot))
            );
            assert_eq!(floating_control_slots(&restored, &saved), None);
            for (is_save, original) in [(true, &saved), (false, &restored)] {
                for bit in 0..original.len() * 8 {
                    let mut changed = original.clone();
                    changed[bit / 8] ^= 1 << (bit % 8);
                    let actual = if is_save {
                        floating_control_slots(&changed, &restored)
                    } else {
                        floating_control_slots(&saved, &changed)
                    };
                    assert_ne!(actual, Some((slot, slot)), "slot {slot}, bit {bit}");
                }
            }
        }
        // An enlarged zero displacement and a negative disp32 must both be rejected.
        let restored = restore(0).unwrap();
        assert_eq!(
            floating_control_slots(&[0x0f, 0xae, 0x5c, 0x24, 0], &restored),
            None
        );
        assert_eq!(
            floating_control_slots(&[0x0f, 0xae, 0x9c, 0x24, 0, 0, 0, 0x80], &restored),
            None
        );
    }
}
