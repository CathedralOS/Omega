use omega_target_operations::{Place, RuntimeStorageRegion, StateGuardOperator};
use psi_arena::HandleSpan;
use std::sync::Arc;

/// Fixed compiler-owned instruction programs whose final encodings can be
/// replayed directly from the target specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerInstructionValidationKind {
    FunctionEnter,
    FunctionReturn,
    DispatchLoopEnter {
        entry_dispatch_index: u32,
    },
    DispatchCaseEnter {
        dispatch_index: u32,
        skip_byte_distance: isize,
    },
    DispatchStaticGuard {
        operator: StateGuardOperator,
        storage_region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        skip_byte_distance: isize,
        is_float: bool,
    },
    PlacePairGuard {
        left: Place,
        right: Place,
        byte_size: usize,
        failure_branch_distance: isize,
        operator: StateGuardOperator,
        is_float: bool,
    },
    PlaceValueGuard {
        place: Place,
        byte_size: usize,
        expected_value: i64,
        failure_branch_distance: isize,
        operator: StateGuardOperator,
    },
    RuntimeTextLiteralGuard {
        buffer_symbol: Arc<str>,
        literal: Arc<str>,
        failure_branch_distances: Vec<isize>,
        delimiter_failure_branch_distance: isize,
    },
    RuntimeTextStorageGuard {
        buffer_symbol: Arc<str>,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        literal_len: usize,
        compare_failure_branch_distance: isize,
        delimiter_failure_branch_distance: isize,
        operator: StateGuardOperator,
    },
    RuntimeValueGuard {
        left: omega_target_operations::RuntimeValueOperandHandle,
        right: omega_target_operations::RuntimeValueOperandHandle,
        byte_size: usize,
        failure_branch_distance: isize,
        operator: StateGuardOperator,
    },
    ReturnRegisterIntegerWrite {
        register: omega_calling_conventions::MachineRegister,
        byte_size: usize,
        value: i64,
    },
    RuntimeStorageToReturnRegister {
        register: omega_calling_conventions::MachineRegister,
        storage_region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    EntryArgumentRegisterWrite {
        register: omega_calling_conventions::MachineRegister,
        byte_offset: usize,
        byte_size: usize,
    },
    EntryStackArgumentWrite {
        stack_byte_offset: u32,
        byte_offset: usize,
        byte_size: usize,
    },
    EntryIndirectArgumentWrite {
        pointer: omega_calling_conventions::IndirectPointerLocation,
        byte_offset: usize,
        byte_size: usize,
    },
    EntryArgumentsSliceDescriptorWrite {
        descriptor_offset: usize,
        spill_offset: usize,
        byte_length: usize,
    },
    ExitIndirectResultCopy {
        source: omega_target_operations::Place,
        target: omega_target_operations::Place,
        byte_count: usize,
    },
    CompilerBodyPlaceCopy {
        source: omega_target_operations::Place,
        target: omega_target_operations::Place,
        byte_count: usize,
    },
    CompilerBodyPlaceIntegerWrite {
        target: Place,
        value: i64,
        byte_size: usize,
    },
    CompilerBodyStorageBitFieldWrite {
        region: RuntimeStorageRegion,
        base_byte_offset: usize,
        fragments: Vec<omega_target_operations::RuntimeBitFieldFragment>,
        value: i64,
    },
    CompilerBodyPlaceBinaryWrite {
        target: Place,
        byte_size: usize,
        left: omega_target_operations::RuntimeValueOperandHandle,
        operator: StateGuardOperator,
        right: omega_target_operations::RuntimeValueOperandHandle,
        is_float: bool,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
        target_signed: bool,
    },
    CompilerBodyStorageConvertWrite {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        target_byte_size: usize,
        source: omega_target_operations::RuntimeValueOperandHandle,
        source_byte_size: usize,
        source_is_float: bool,
        target_is_float: bool,
        source_signed: bool,
        target_signed: bool,
        trapping: bool,
        saturating: bool,
    },
    CompilerBodyPlaceConvertWrite {
        target: Place,
        target_byte_size: usize,
        source: omega_target_operations::RuntimeValueOperandHandle,
        source_byte_size: usize,
        source_is_float: bool,
        target_is_float: bool,
        source_signed: bool,
        target_signed: bool,
        trapping: bool,
        saturating: bool,
    },
    DispatchStateWrite {
        dispatch_index: u32,
        case_leave_byte_distance: isize,
    },
    DispatchForwardBranchSkip {
        branch_arms_end_byte_distance: isize,
    },
    DispatchCaseLeave {
        loop_byte_distance: isize,
    },
}

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
    MachineIndexed {
        base_byte_offset: u32,
        index_from_frame: bool,
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
        register: psi_language_core::inline_assembly::AsmControlRegister,
        destination_byte_offset: u32,
    },
    ControlRegisterWrite {
        register: psi_language_core::inline_assembly::AsmControlRegister,
        source_operand_byte_width: u32,
    },
    FlagsSnapshot {
        destination_byte_offset: u32,
    },
    FlagsRestore {
        source_operand_byte_width: u32,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncodedMachineInstruction {
    pub selected_instruction_index: u32,
    pub bytes: HandleSpan<u8>,
    pub compiler_validation_kind: Option<CompilerInstructionValidationKind>,
    pub checked_validation_kind: Option<CheckedInstructionValidationKind>,
    /// Semantic loader checks known independently from the privileged-opcode
    /// envelope. `None` entries are unused; complex operand trees remain
    /// outside the completed final-byte certificate until their decoder lands.
    pub checked_operand_loaders: [Option<CheckedOperandLoaderValidation>; 2],
}
