pub fn x86_64_machine_register_encoding(
    register: calling_conventions::MachineRegister,
) -> Option<u8> {
    Some(match register {
        calling_conventions::MachineRegister::X86Rax => 0,
        calling_conventions::MachineRegister::X86Rcx => 1,
        calling_conventions::MachineRegister::X86Rdx => 2,
        calling_conventions::MachineRegister::X86Rbx => 3,
        calling_conventions::MachineRegister::X86Rsi => 6,
        calling_conventions::MachineRegister::X86Rdi => 7,
        calling_conventions::MachineRegister::X86R8 => 8,
        calling_conventions::MachineRegister::X86R9 => 9,
        calling_conventions::MachineRegister::X86R10 => 10,
        calling_conventions::MachineRegister::X86R11 => 11,
        calling_conventions::MachineRegister::X86R12 => 12,
        calling_conventions::MachineRegister::X86R13 => 13,
        calling_conventions::MachineRegister::X86R14 => 14,
        calling_conventions::MachineRegister::X86R15 => 15,
        _ => return None,
    })
}
