use omega_core::arena::HandleSpan;

/// Checked-assembly instructions whose privilege-bearing final encoding can be
/// validated independently from the encoder.
///
/// This tag is retained beside the encoded byte span so final-image validation
/// never has to rediscover instruction boundaries by scanning arbitrary bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedInstructionValidationKind {
    MachineHalt,
    LoadFence,
    StoreFence,
    FullFence,
    InterruptDisable,
    InterruptEnable,
    /// `out dx, al` with a compile-time-known port. The value loader may still
    /// be runtime- or relocation-backed; the final validator binds the exact
    /// privileged destination and the closed register/opcode envelope.
    PortWriteImmediatePort {
        port: u16,
    },
    /// `in al, dx` with a compile-time-known port and a compiler-owned
    /// relocated destination store.
    PortReadImmediatePort {
        port: u16,
        destination_byte_offset: u32,
    },
    MsrReadImmediateIndex {
        index: u32,
        destination_byte_offset: u32,
    },
    MsrWriteImmediateIndex {
        index: u32,
    },
    ControlRegisterRead {
        register: omega_core::inline_assembly::AsmControlRegister,
        destination_byte_offset: u32,
    },
    ControlRegisterWrite {
        register: omega_core::inline_assembly::AsmControlRegister,
    },
    FlagsSnapshot {
        destination_byte_offset: u32,
    },
    FlagsRestore,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodedMachineInstruction {
    pub selected_instruction_index: u32,
    pub bytes: HandleSpan<u8>,
    pub checked_validation_kind: Option<CheckedInstructionValidationKind>,
}
