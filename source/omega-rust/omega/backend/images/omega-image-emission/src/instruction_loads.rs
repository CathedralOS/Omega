use omega_calling_conventions::MachineRegister;

pub(super) fn x86_terminal_register(register: MachineRegister) -> Option<u8> {
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

pub(super) fn aarch64_terminal_register(register: MachineRegister) -> Option<u8> {
    match register {
        MachineRegister::Aarch64X(value @ 0..=30) => Some(value),
        _ => None,
    }
}

pub(super) fn expected_x86_stack_load(
    bytes: &mut Vec<u8>,
    register: u8,
    offset: u32,
    width: u16,
) -> Option<()> {
    x86_replay_rsp_load(bytes, register, offset, width)
}

pub(super) fn expected_x86_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    offset: u32,
    width: u16,
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
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    Some(())
}

pub(super) fn expected_aarch64_stack_load(register: u8, offset: u32, width: u16) -> Option<u32> {
    aarch64_replay_stack_load(register, offset, width)
}

pub(super) fn expected_aarch64_memory_load(
    register: u8,
    base: u8,
    offset: u32,
    width: u16,
) -> Option<u32> {
    if width != 8 || !offset.is_multiple_of(8) || offset / 8 > 0xfff {
        return None;
    }
    Some(0xf940_0000 | ((offset / 8) << 10) | (u32::from(base) << 5) | u32::from(register))
}

pub(super) fn x86_replay_rsp_load(
    bytes: &mut Vec<u8>,
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<()> {
    match byte_size {
        1 => {
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.extend_from_slice(&[0x0f, 0xb6]);
        }
        8 => {
            bytes.push(0x48 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        4 => {
            bytes.push(0x40 | (((register >> 3) & 1) << 2));
            bytes.push(0x8b);
        }
        _ => return None,
    }
    if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, byte_offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
    Some(())
}

pub(super) fn x86_replay_memory_load(
    bytes: &mut Vec<u8>,
    destination: u8,
    base: u8,
    byte_offset: u32,
) {
    bytes.push(0x40 | (((destination >> 3) & 1) << 2) | ((base >> 3) & 1));
    bytes.extend_from_slice(&[0x0f, 0xb6]);
    if byte_offset == 0 && (base & 7) != 5 {
        bytes.push(((destination & 7) << 3) | (base & 7));
    } else if byte_offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[
            0x40 | ((destination & 7) << 3) | (base & 7),
            byte_offset as u8,
        ]);
    } else {
        bytes.push(0x80 | ((destination & 7) << 3) | (base & 7));
        bytes.extend_from_slice(&byte_offset.to_le_bytes());
    }
}

pub(super) fn aarch64_replay_stack_load(
    register: u8,
    byte_offset: u32,
    byte_size: u16,
) -> Option<u32> {
    let scale = u32::from(byte_size);
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

pub(super) fn aarch64_replay_memory_load(register: u8, base: u8, byte_offset: u32) -> Option<u32> {
    (byte_offset <= 0xfff)
        .then_some(0x3940_0000 | (byte_offset << 10) | (u32::from(base) << 5) | u32::from(register))
}
