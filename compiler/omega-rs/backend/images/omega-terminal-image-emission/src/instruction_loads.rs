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
    if width != 8 {
        return None;
    }
    bytes.push(0x48 | (((register >> 3) & 1) << 2));
    bytes.push(0x8b);
    if offset <= i8::MAX as u32 {
        bytes.extend_from_slice(&[0x44 | ((register & 7) << 3), 0x24, offset as u8]);
    } else {
        bytes.extend_from_slice(&[0x84 | ((register & 7) << 3), 0x24]);
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    Some(())
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
    if width != 8 || !offset.is_multiple_of(8) || offset / 8 > 0xfff {
        return None;
    }
    Some(0xf940_0000 | ((offset / 8) << 10) | (31 << 5) | u32::from(register))
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
