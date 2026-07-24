use omega_core::arena::HandleSpan;

/// The only registers the x86 checked-assembly operand evaluator may target.
/// This is retained as semantic validation input rather than rediscovered from
/// arbitrary final bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedOperandLoaderRegister {
    R10,
    R11,
}

/// A closed leaf of the checked-assembly runtime-value operand vocabulary that
/// final-image validation can decode independently from the encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedOperandLoaderKind {
    Immediate {
        value: u64,
    },
    Storage {
        byte_offset: u32,
        byte_size: u8,
    },
    Pointee {
        pointer_byte_offset: u32,
        field_byte_offset: u32,
        byte_size: u8,
    },
    FrameFixedIndexed {
        descriptor_byte_offset: u32,
        element_index: u64,
        element_byte_size: u32,
        field_byte_offset: u32,
        byte_size: u8,
    },
    FrameBaseIndexed {
        base_byte_offset: u32,
        index_byte_offset: u32,
        index_byte_size: u8,
        element_byte_size: u32,
        field_byte_offset: u32,
        byte_size: u8,
    },
    FrameIndexed {
        descriptor_byte_offset: u32,
        index_from_machine: bool,
        index_byte_offset: u32,
        index_byte_size: u8,
        element_byte_size: u32,
        field_byte_offset: u32,
        byte_size: u8,
    },
}

/// One operand loader's exact subspan and expected semantic meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedOperandLoaderValidation {
    pub byte_offset: u32,
    pub byte_width: u32,
    pub register: CheckedOperandLoaderRegister,
    pub kind: CheckedOperandLoaderKind,
}

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
        value_operand_byte_width: u32,
    },
    /// `in al, dx` with a compile-time-known port and a compiler-owned
    /// relocated destination store.
    PortReadImmediatePort {
        port: u16,
        destination_byte_offset: u32,
    },
    /// `out dx, al` whose port and value are runtime operands. Their encoded
    /// widths retain the exact boundaries around the fixed register-transfer
    /// and privileged-opcode skeleton.
    PortWriteRuntimePort {
        port_operand_byte_width: u32,
        value_operand_byte_width: u32,
    },
    /// `in al, dx` whose port is a runtime operand. The destination remains a
    /// compiler-owned relocated store.
    PortReadRuntimePort {
        port_operand_byte_width: u32,
        destination_byte_offset: u32,
    },
    MsrReadImmediateIndex {
        index: u32,
        destination_byte_offset: u32,
    },
    MsrWriteImmediateIndex {
        index: u32,
        value_operand_byte_width: u32,
    },
    MsrReadRuntimeIndex {
        index_operand_byte_width: u32,
        destination_byte_offset: u32,
    },
    MsrWriteRuntimeIndex {
        index_operand_byte_width: u32,
        value_operand_byte_width: u32,
    },
    ControlRegisterRead {
        register: omega_core::inline_assembly::AsmControlRegister,
        destination_byte_offset: u32,
    },
    ControlRegisterWrite {
        register: omega_core::inline_assembly::AsmControlRegister,
        source_operand_byte_width: u32,
    },
    FlagsSnapshot {
        destination_byte_offset: u32,
    },
    FlagsRestore {
        source_operand_byte_width: u32,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EncodedMachineInstruction {
    pub selected_instruction_index: u32,
    pub bytes: HandleSpan<u8>,
    pub checked_validation_kind: Option<CheckedInstructionValidationKind>,
    /// Semantic loader checks known independently from the privileged-opcode
    /// envelope. `None` entries are unused; complex operand trees remain
    /// outside the completed final-byte certificate until their decoder lands.
    pub checked_operand_loaders: [Option<CheckedOperandLoaderValidation>; 2],
}
