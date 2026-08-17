use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};

/// Bytes reserved by the fixed ordinary x86-64 frame. Saving eight registers
/// plus one 16-byte control-state slot keeps the entry stack's modulo-16
/// alignment unchanged, so the existing SysV and Microsoft x64 outbound-call
/// reservations remain valid. The slot retains the caller's complete MXCSR;
/// only Omega's canonical value is live while checked code executes.
pub const FUNCTION_FRAME_BYTES: usize = 80;

/// Save/restore envelope placed around a returning foreign call. Its private
/// 16-byte slot preserves stack alignment while retaining the complete MXCSR.
pub const FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH: usize = 8;
pub const FOREIGN_FLOAT_CONTROL_SUFFIX_WIDTH: usize = 8;

pub fn encode_foreign_float_control_prefix_bytes() -> [u8; FOREIGN_FLOAT_CONTROL_PREFIX_WIDTH] {
    [
        0x48, 0x83, 0xec, 0x10, // sub rsp, 16
        0x0f, 0xae, 0x1c, 0x24, // stmxcsr [rsp]
    ]
}

pub fn encode_foreign_float_control_suffix_bytes() -> [u8; FOREIGN_FLOAT_CONTROL_SUFFIX_WIDTH] {
    [
        0x0f, 0xae, 0x14, 0x24, // ldmxcsr [rsp]
        0x48, 0x83, 0xc4, 0x10, // add rsp, 16
    ]
}

pub fn function_enter_width() -> usize {
    33
}

/// Preserve the union of the SysV AMD64 and Microsoft x64 nonvolatile GPRs
/// used by generated Omega code: rbx, rbp, rsi, rdi, and r12-r15. The extra
/// aligned slot saves the incoming MXCSR before installing `0x1f80`: masked
/// exceptions, nearest-even rounding, and gradual underflow (FTZ/DAZ clear).
pub fn encode_function_enter_bytes() -> [u8; 33] {
    [
        0x53, // push rbx
        0x55, // push rbp
        0x56, // push rsi
        0x57, // push rdi
        0x41, 0x54, // push r12
        0x41, 0x55, // push r13
        0x41, 0x56, // push r14
        0x41, 0x57, // push r15
        0x48, 0x83, 0xec, 0x10, // sub rsp, 16
        0x0f, 0xae, 0x1c, 0x24, // stmxcsr [rsp]
        0xc7, 0x44, 0x24, 0x04, 0x80, 0x1f, 0x00, 0x00, // mov dword [rsp+4], 0x1f80
        0x0f, 0xae, 0x54, 0x24, 0x04, // ldmxcsr [rsp+4]
    ]
}

pub fn return_width() -> usize {
    21
}

pub fn encode_return_bytes() -> [u8; 21] {
    [
        0x0f, 0xae, 0x14, 0x24, // ldmxcsr [rsp]
        0x48, 0x83, 0xc4, 0x10, // add rsp, 16
        0x41, 0x5f, // pop r15
        0x41, 0x5e, // pop r14
        0x41, 0x5d, // pop r13
        0x41, 0x5c, // pop r12
        0x5f, // pop rdi
        0x5e, // pop rsi
        0x5d, // pop rbp
        0x5b, // pop rbx
        0xc3, // ret
    ]
}

/// Register writes performed by the ordinary x86-64 function-entry sequence.
/// Pushes only update SP; the stored nonvolatile register values are reads.
pub fn function_enter_register_writes() -> RegisterSet {
    RegisterSet::default()
}

pub fn function_enter_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([MachineState::StackPointer, MachineState::ControlState])
}

/// Exact state written while restoring the fixed frame and returning. The
/// explicit RSP identity is retained in addition to its stack-pointer class.
pub fn return_register_writes() -> RegisterSet {
    RegisterSet::new([
        MachineRegister::X86Rbx,
        MachineRegister::X86Rsp,
        MachineRegister::X86Rbp,
        MachineRegister::X86Rsi,
        MachineRegister::X86Rdi,
        MachineRegister::X86R12,
        MachineRegister::X86R13,
        MachineRegister::X86R14,
        MachineRegister::X86R15,
    ])
}

pub fn return_additional_machine_state() -> MachineStateSet {
    MachineStateSet::new([
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::ControlState,
    ])
}
