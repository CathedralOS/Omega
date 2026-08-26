use omega_target_operations::{Place, RuntimeStorageRegion, StateGuardOperator};
use psi_arena::HandleSpan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerRuntimeImportSubcall {
    pub library: Arc<str>,
    pub symbol: Arc<str>,
    pub plan: omega_calling_conventions::CallPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompilerInstructionWireScalarRange {
    pub minimum: i64,
    pub maximum: i64,
    pub signed: bool,
}

/// Closed compiler-owned atomic programs retained for independent final-byte,
/// relocation, and StatePlan replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilerInstructionAtomicOperation {
    Load {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    Store {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        value: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    FetchAdd {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        delta: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    FetchSub {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        delta: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    FetchXor {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        value: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    FetchOr {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        value: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    FetchAnd {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        value: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    Swap {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        new_value: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    CompareExchange {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        expected: omega_target_operations::RuntimeValueOperandHandle,
        new_value: omega_target_operations::RuntimeValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
}

/// Fixed compiler-owned instruction programs whose final encodings can be
/// replayed directly from the target specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerInstructionValidationKind {
    FunctionEnter,
    FunctionReturn,
    InternalFunctionCall {
        target: omega_control_flow::MachineFunctionIdentity,
    },
    OutgoingStackAddressLoad {
        register: omega_calling_conventions::MachineRegister,
        stack_byte_offset: u32,
    },
    OutgoingStackFrameReserve {
        byte_count: u32,
    },
    OutgoingStackU64Write {
        stack_byte_offset: u32,
        value: u64,
    },
    EntryIndirectU64ToOutgoingStackCopy {
        source_register: omega_calling_conventions::MachineRegister,
        source_byte_offset: u32,
        stack_byte_offset: u32,
    },
    OutgoingStackFrameRelease {
        byte_count: u32,
    },
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
        literal: Arc<[u8]>,
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
    CompilerBodyAtomic(CompilerInstructionAtomicOperation),
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
    CompilerBodyPlaceAddressWrite {
        source: Place,
        target_offset: usize,
    },
    CompilerBodyConstantHostResult {
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        result_byte_size: usize,
        value: i64,
    },
    CompilerBodyOutboundImmediateImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundImmediateImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundFloatImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundDereferencedImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundDataImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundDataImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredFloatImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredFloatImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredAggregateImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredAggregateImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundAuthoredAggregateResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        locator: omega_calling_conventions::HostImportLocator,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundIndirectCall {
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        mechanism: omega_calling_conventions::HostBindingMechanism,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundOpenCreateImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyRuntimeByteRead {
        operation_key: omega_calling_conventions::HostOperationKey,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        payload_offset: usize,
        mechanism: omega_calling_conventions::HostBindingMechanism,
        plan: omega_calling_conventions::CallPlan,
        get_std_handle: Option<CompilerRuntimeImportSubcall>,
    },
    CompilerBodyRuntimeByteWrite {
        operation_key: omega_calling_conventions::HostOperationKey,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        literal_symbol: Arc<str>,
        source_is_place: bool,
        mechanism: omega_calling_conventions::HostBindingMechanism,
        plan: omega_calling_conventions::CallPlan,
        get_std_handle: Option<CompilerRuntimeImportSubcall>,
    },
    CompilerBodyRuntimeLineRead {
        operation_key: omega_calling_conventions::HostOperationKey,
        buffer_symbol: Arc<str>,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_capacity: usize,
        target: omega_target_operations::RuntimeTextReadTarget,
        mechanism: omega_calling_conventions::HostBindingMechanism,
        plan: omega_calling_conventions::CallPlan,
        get_std_handle: Option<CompilerRuntimeImportSubcall>,
    },
    CompilerBodyOutboundStorageImport {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundStorageImportResult {
        operation_key: omega_calling_conventions::HostOperationKey,
        operands: Vec<omega_target_operations::InstructionOperand>,
        library: Arc<str>,
        symbol: Arc<str>,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscall {
        operands: Vec<omega_target_operations::InstructionOperand>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallDataArguments {
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallResult {
        operands: Vec<omega_target_operations::InstructionOperand>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallResultDataArguments {
        operands: Vec<omega_target_operations::InstructionOperand>,
        data_symbols: Vec<Arc<str>>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallResultStorageArguments {
        operands: Vec<omega_target_operations::InstructionOperand>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallStorageArguments {
        operands: Vec<omega_target_operations::InstructionOperand>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallTimespecArgument {
        operands: Vec<omega_target_operations::InstructionOperand>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyOutboundSyscallTimespecResult {
        operands: Vec<omega_target_operations::InstructionOperand>,
        number: u32,
        plan: omega_calling_conventions::CallPlan,
    },
    CompilerBodyStorageBitFieldWrite {
        region: RuntimeStorageRegion,
        base_byte_offset: usize,
        fragments: Vec<omega_target_operations::RuntimeBitFieldFragment>,
        value: i64,
    },
    CompilerBodyPlaceBoundedBufferWrite {
        target: Place,
        literal: Arc<[u8]>,
    },
    CompilerBodyPlaceBoundedBufferLiteralAppend {
        target: Place,
        literal: Arc<[u8]>,
    },
    CompilerBodyPlaceBoundedBufferSourceAppend {
        target: Place,
        source: Place,
    },
    CompilerBodyPlaceStringWrite {
        target: Place,
        data_symbol: Arc<str>,
        byte_length: usize,
    },
    CompilerBodyWireLiteralByteAppend {
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
        value: u8,
    },
    CompilerBodyWireScalarVarintAppend {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        byte_size: usize,
        zigzag: bool,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    CompilerBodyWireTextBytesAppend {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        out_length: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    CompilerBodyWireScalarSliceAppend {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        element_byte_size: usize,
        zigzag: bool,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        out_length: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    CompilerBodyWireRepeatedScalarVarintAppend {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        byte_size: usize,
        zigzag: bool,
        index: u64,
        count_region: RuntimeStorageRegion,
        count_offset: usize,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    CompilerBodyWireExpectedByteRead {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        expected: u8,
    },
    CompilerBodyWireScalarVarintRead {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        zigzag: bool,
        range: Option<CompilerInstructionWireScalarRange>,
    },
    CompilerBodyWireByteSliceRead {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        predicate_mask: u8,
    },
    CompilerBodyWireNestedOpen {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        end_region: RuntimeStorageRegion,
        end_offset: usize,
    },
    CompilerBodyWireNestedClose {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        end_region: RuntimeStorageRegion,
        end_offset: usize,
    },
    CompilerBodyWireRepeatedScalarVarintRead {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        end_region: RuntimeStorageRegion,
        end_offset: usize,
        count_region: RuntimeStorageRegion,
        count_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        zigzag: bool,
        range: Option<CompilerInstructionWireScalarRange>,
    },
    CompilerBodyTextBufferMaterialize {
        buffer_symbol: Arc<str>,
        target: Place,
    },
    CompilerBodyTextLiteralAppend {
        buffer_symbol: Arc<str>,
        target: Place,
        literal: Arc<[u8]>,
    },
    CompilerBodyTextStoredAppend {
        buffer_symbol: Arc<str>,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target: Place,
    },
    CompilerBodyTextLiteralSegmentWrite {
        buffer_symbol: Arc<str>,
        byte_offset: usize,
        literal: Arc<[u8]>,
    },
    CompilerBodyTextStoredSuffixAppend {
        buffer_symbol: Arc<str>,
        buffer_offset: usize,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        length_delta: usize,
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
