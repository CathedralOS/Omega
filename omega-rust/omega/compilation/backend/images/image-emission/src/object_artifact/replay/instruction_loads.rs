//! Register numbering and exact stack-load instruction replay shared by the
//! replay passes on both supported architectures.

use calling_conventions::MachineRegister;

pub(crate) fn x86_terminal_register(register: MachineRegister) -> Option<u8> {
    use MachineRegister::*;
    Some(match register {
        X86Rax => 0,
        X86Rcx => 1,
        X86Rdx => 2,
        X86Rbx => 3,
        X86Rsp => 4,
        X86Rbp => 5,
        X86Rsi => 6,
        X86Rdi => 7,
        X86R8 => 8,
        X86R9 => 9,
        X86R10 => 10,
        X86R11 => 11,
        X86R12 => 12,
        X86R13 => 13,
        X86R14 => 14,
        X86R15 => 15,
        _ => return None,
    })
}

pub(crate) fn aarch64_terminal_register(register: MachineRegister) -> Option<u8> {
    match register {
        MachineRegister::Aarch64X(value @ 0..=30) => Some(value),
        _ => None,
    }
}

pub(crate) fn expected_x86_stack_load(
    bytes: &mut Vec<u8>,
    register: u8,
    offset: u32,
    width: u32,
) -> Option<()> {
    x86_replay_rsp_load(bytes, register, offset, width)
}

pub(crate) fn expected_x86_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    offset: u32,
    width: u32,
) -> Option<()> {
    if width != 8 {
        return None;
    }
    bytes.push(0x48 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
    bytes.push(0x8b);
    if offset == 0 && (base & 7) != 5 {
        bytes.push(((destination & 7) << 3) | (base & 7));
    } else if offset <= i8::MAX as u32 {
        bytes.push(0x40 | ((destination & 7) << 3) | (base & 7));
        bytes.push(offset as u8);
    } else {
        bytes.push(0x80 | ((destination & 7) << 3) | (base & 7));
        crate::codec_bytes::push_encoded(bytes, offset.to_le_bytes());
    }
    Some(())
}

pub(crate) fn expected_aarch64_stack_load(register: u8, offset: u32, width: u32) -> Option<u32> {
    aarch64_replay_stack_load(register, offset, width)
}

pub(crate) fn expected_aarch64_memory_load(
    register: u8,
    base: u8,
    offset: u32,
    width: u32,
) -> Option<u32> {
    if width != 8 || !offset.is_multiple_of(8) || offset / 8 > 0xfff {
        return None;
    }
    Some(0xf940_0000 | ((offset / 8) << 10) | (u32::from(base) << 5) | u32::from(register))
}

/// Append the REX (and operand-size) prefix selecting `byte_size` for an x86-64
/// `mov`, taking REX.R from `register` and REX.B from `base`.
fn x86_mov_prefix(bytes: &mut Vec<u8>, register: u8, base: u8, byte_size: u32) -> Option<()> {
    let extension = (((register >> 3) & 1) << 2) | ((base >> 3) & 1);
    match byte_size {
        1 | 4 => bytes.push(0x40 | extension),
        2 => bytes.extend_from_slice(&[0x66, 0x40 | extension]),
        8 => bytes.push(0x48 | extension),
        _ => return None,
    }
    Some(())
}

/// Append the ModRM byte and displacement addressing `[rsp + byte_offset]`.
fn x86_rsp_modrm(bytes: &mut Vec<u8>, register: u8, byte_offset: u32) {
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
    }
}

/// Append the ModRM byte and displacement addressing `[base + byte_offset]`.
fn x86_memory_modrm(bytes: &mut Vec<u8>, register: u8, base: u8, byte_offset: u32) {
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((register & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x40 | ((register & 7) << 3) | (base & 7), byte_offset as u8]);
    } else {
        bytes.push(0x80 | ((register & 7) << 3) | (base & 7));
        crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
    }
}

pub(crate) fn x86_replay_rsp_load(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u32,
) -> Option<()> {
    x86_mov_prefix(bytes, register, 0, byte_size)?;
    match byte_size {
        1 => bytes.extend_from_slice(&[0x0f, 0xb6]),
        2 => bytes.extend_from_slice(&[0x0f, 0xb7]),
        4 | 8 => bytes.push(0x8b),
        _ => return None,
    }
    x86_rsp_modrm(bytes, register, byte_offset);
    Some(())
}

/// Append the `mov [rsp + byte_offset], register` of `byte_size` bytes.
pub(crate) fn x86_replay_stack_store(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u32,
) -> Option<()> {
    x86_mov_prefix(bytes, register, 0, byte_size)?;
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    x86_rsp_modrm(bytes, register, byte_offset);
    Some(())
}

/// Append the `mov [base + byte_offset], register` of `byte_size` bytes.
pub(crate) fn x86_replay_memory_store(
    bytes: &mut Vec<u8>,
    register: u8,
    base: u8,
    byte_offset: u32,
    byte_size: u32,
) -> Option<()> {
    x86_mov_prefix(bytes, register, base, byte_size)?;
    bytes.push(if byte_size == 1 { 0x88 } else { 0x89 });
    x86_memory_modrm(bytes, register, base, byte_offset);
    Some(())
}

pub(crate) fn x86_replay_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    byte_offset: u32,
) {
    bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
    bytes.extend_from_slice(&[0x0f, 0xb6]);
    x86_memory_modrm(bytes, destination, base, byte_offset);
}

pub(crate) fn aarch64_replay_stack_load(
    register: u8,
    byte_offset: u32,
    byte_size: u32,
) -> Option<u32> {
    let scale = byte_size;
    let base = match byte_size {
        1 => 0x3940_0000,
        2 => 0x7940_0000,
        4 => 0xb940_0000,
        8 => 0xf940_0000,
        _ => return None,
    };
    (scale != 0 && byte_offset.is_multiple_of(scale) && byte_offset / scale <= 0xfff)
        .then_some(base | ((byte_offset / scale) << 10) | (31 << 5) | u32::from(register))
}

pub(crate) fn aarch64_replay_memory_load(register: u8, base: u8, byte_offset: u32) -> Option<u32> {
    (byte_offset <= 0xfff)
        .then_some(0x3940_0000 | (byte_offset << 10) | (u32::from(base) << 5) | u32::from(register))
}
