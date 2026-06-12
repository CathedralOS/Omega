use crate::{
    AbstractDataObjectHandle, AbstractValueOperandHandle, InstructionOperand, RuntimeStorageRegion,
    StateGuardLowering, StateGuardOperator,
};
use omega_calling_conventions::HostCapability;
use omega_core::arena::HandleSpan;
use std::sync::Arc;

mod classification;
#[cfg(test)]
mod tests;

pub use classification::AbstractOperationDomain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractOperationKind {
    EnterFunction,
    EnterDispatchLoop {
        entry_dispatch_index: u32,
        terminal_dispatch_index: u32,
    },
    EnterDispatchCase {
        dispatch_index: u32,
    },
    EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering,
        operator: StateGuardOperator,
        storage_region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        has_storage: bool,
        /// When set, the compared storage holds an f64 and the static compare is
        /// performed with `comisd` (whose CF/ZF mirror an unsigned `cmp`, so the
        /// unsigned failure-branch conditions apply).
        is_float: bool,
    },
    CompareRuntimeTextLiteral {
        buffer: AbstractDataObjectHandle,
        literal: Arc<str>,
    },
    CompareRuntimeTextStorage {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorage {
        left_region: RuntimeStorageRegion,
        left_offset: usize,
        right_region: RuntimeStorageRegion,
        right_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
        /// Both operands are f64 and the compare uses `ucomisd` (whose CF/ZF
        /// mirror an unsigned `cmp`, so the unsigned failure conditions apply).
        is_float: bool,
    },
    CompareRuntimeStorageValue {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    CompareRuntimeValues {
        left: AbstractValueOperandHandle,
        right: AbstractValueOperandHandle,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: AbstractDataObjectHandle,
        literal: Arc<str>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: AbstractDataObjectHandle,
        byte_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextStoredSuffix {
        buffer: AbstractDataObjectHandle,
        buffer_offset: usize,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        length_delta: usize,
    },
    MaterializeRuntimeTextBuffer {
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimePointee {
        buffer: AbstractDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
        buffer: AbstractDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimePointee {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimePointee {
        buffer: AbstractDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimeFrameIndexed {
        buffer: AbstractDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        literal: Arc<str>,
    },
    /// compact_binary v0 wire framing (chapter 20): store one COMPILE-TIME
    /// byte (era and field-tag varint bytes are known when the schema is) into
    /// the encode buffer at the stored cursor, then advance the cursor by one.
    /// The cursor lives in the caller's `written` out-parameter slot, so a
    /// finished encode sequence leaves the total byte count behind.
    AppendWireLiteralByte {
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
        value: u8,
    },
    /// compact_binary v0 wire framing (chapter 20): LEB128-encode a RUNTIME
    /// scalar into the encode buffer at the stored cursor, advancing the
    /// cursor by the encoded byte count (1..=10).
    AppendWireScalarVarint {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        /// Width of the runtime scalar load: 1 (bool), 4, or 8 bytes.
        byte_size: usize,
        /// Signed sources sign-extend to 64 bits and zigzag
        /// (`(n << 1) ^ (n >> 63)`) so small negatives stay short on the wire.
        zigzag: bool,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    /// compact_binary v0 wire framing (chapter 20): append a RUNTIME `String`
    /// field -- the value is a `{ptr @ +0, len @ +8}` text descriptor at the
    /// source place -- as the len LEB128 varint followed by len raw bytes
    /// copied from ptr. The length varint is capacity-covered by validation's
    /// worst-case budget (String fields encode LAST); the byte-copy is the one
    /// append whose size is runtime-unbounded, so it alone bounds every store
    /// against `out_length` (the buffer's compile-time byte length) and DROPS
    /// content past capacity -- the cursor stops at `out_length`, never past.
    AppendWireTextBytes {
        source_region: RuntimeStorageRegion,
        /// Byte offset of the text descriptor (ptr at +0, len at +8).
        source_offset: usize,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        /// Compile-time byte length of the encode buffer (`[u8; N]`); every
        /// byte-copy store is bounds-checked against it.
        out_length: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    /// compact_binary v0 wire decoding (chapter 20, wire stage 2b): expect one
    /// COMPILE-TIME framing byte (era and field-tag varint bytes are known
    /// when the schema is) at the stored cursor. The cursor lives in the
    /// caller's `read` out-parameter slot; the sticky success flag lives in
    /// the `ok` slot. A cursor at/after `buffer_length` clears `ok` without
    /// consuming; a mismatching byte consumes one byte and clears `ok`. The
    /// flag is only ever CLEARED here, so a failed decode stays failed no
    /// matter what later steps read.
    ReadWireExpectedByte {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        /// Compile-time byte length of the decode buffer (`[u8; N]`); every
        /// byte read is bounds-checked against it.
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        expected: u8,
    },
    /// compact_binary v0 wire decoding (chapter 20, wire stage 2b): LEB128-read
    /// a RUNTIME scalar at the stored cursor into the target place, advancing
    /// the cursor by the consumed byte count. Truncated input (cursor past
    /// `buffer_length` mid-varint) and overlong varints (more than ten groups,
    /// i.e. a continuation past shift 63) clear the sticky `ok` flag; the
    /// target is stored regardless (failed decodes leave unspecified field
    /// contents -- the contract is `ok`, not the partial payload).
    ReadWireScalarVarint {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        /// Width of the runtime scalar store: 1 (bool), 4, or 8 bytes; wider
        /// decoded values truncate to the field width.
        byte_size: usize,
        /// Signed targets un-zigzag (`(n >> 1) ^ -(n & 1)`) after the read.
        zigzag: bool,
    },
    /// compact_binary v0 wire decoding (chapter 20, nested message fields):
    /// turn the sub-message LENGTH just read into the `end` slot into an
    /// ABSOLUTE end bound (`end += cursor`) and clear the sticky `ok` flag
    /// when the bound exceeds the buffer's compile-time length. Runs after
    /// the nested field's length varint, before its field reads; the cursor
    /// does not move.
    ReadWireNestedOpen {
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
    /// compact_binary v0 wire decoding (chapter 20, nested message fields):
    /// clear the sticky `ok` flag unless the cursor landed EXACTLY on the end
    /// bound the matching `ReadWireNestedOpen` stored -- the declared
    /// sub-message length must equal the bytes its fields consumed. The
    /// cursor does not move.
    ReadWireNestedClose {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        end_region: RuntimeStorageRegion,
        end_offset: usize,
    },
    /// compact_binary v0 wire framing (chapter 20, repeated fields): append
    /// element `index` of a packed repeated field ONLY IF `index < count`
    /// (the runtime element count in the value's count-companion slot, read
    /// as an unsigned 64-bit `usize`). A repeated field's element count is
    /// runtime-sized but bounded by the schema's declared maximum, so
    /// selection unrolls the maximum and guards each append -- the emitted
    /// width stays compile-time-fixed (the widths invariant) while the
    /// payload reflects the live count. A skipped append leaves the cursor
    /// untouched.
    AppendWireRepeatedScalarVarint {
        source_region: RuntimeStorageRegion,
        /// Byte offset of element `index`'s slot (array base + index * size).
        source_offset: usize,
        /// Width of the runtime scalar load: 1 (bool), 4, or 8 bytes.
        byte_size: usize,
        /// Signed sources sign-extend to 64 bits and zigzag before the emit.
        zigzag: bool,
        /// This element's compile-time position in the repeated field.
        index: u64,
        count_region: RuntimeStorageRegion,
        count_offset: usize,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    /// compact_binary v0 wire decoding (chapter 20, repeated fields): read
    /// one packed element ONLY IF the cursor sits strictly BELOW the end
    /// bound the surrounding `ReadWireNestedOpen` stored; on the taken path,
    /// LEB128-read a scalar into the target slot (sticky-ok semantics
    /// identical to `ReadWireScalarVarint`) and increment the
    /// count-companion slot. Selection unrolls the declared maximum, so a
    /// payload packing more elements than the maximum leaves the cursor
    /// short of the bound and the closing `ReadWireNestedClose` clears `ok`
    /// -- the hostile-count cap. A skipped read changes nothing (cursor, ok,
    /// target, and count all stay put).
    ReadWireRepeatedScalarVarint {
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
        /// Byte offset of this element's slot (array base + index * size).
        target_offset: usize,
        /// Width of the runtime scalar store: 1 (bool), 4, or 8 bytes.
        byte_size: usize,
        /// Signed targets un-zigzag after the read.
        zigzag: bool,
    },
    WriteRuntimeMachineInteger {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeStorageInteger {
        target_region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimePointeeInteger {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeStorageBinary {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
        /// When set, the operands carry IEEE-754 bit patterns and the operation
        /// is performed on the SSE/XMM unit (`movq`+`addsd`/...) instead of the
        /// integer ALU.
        is_float: bool,
    },
    /// A numeric `as` cast: load `source` into a register, convert it between
    /// integer and floating-point representations (`cvttsd2si`/`cvtsi2sd`/
    /// `cvtsd2ss`/`cvtss2sd`, or a sized integer move), then store the result.
    WriteRuntimeStorageConvert {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        target_byte_size: usize,
        source: AbstractValueOperandHandle,
        source_byte_size: usize,
        source_is_float: bool,
        target_is_float: bool,
        /// Whether the integer source is signed (drives sign- vs zero-extension
        /// of a narrow source and the signedness of an int->float conversion).
        source_signed: bool,
    },
    WriteRuntimePointeeBinary {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
    },
    WriteRuntimeFrameIndexedInteger {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeFrameBaseIndexedInteger {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeMachineIndexedInteger {
        base_byte_offset: usize,
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeFrameIndexedBinary {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
    },
    WriteRuntimeFrameBaseIndexedBinary {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
    },
    WriteRuntimeMachineString {
        byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeFrameString {
        byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimePointeeString {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeFrameIndexedString {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeMachineIndexedString {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeStorageAddressToRuntimeFrame {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_offset: usize,
    },
    WriteRuntimePointeeAddressToRuntimeFrame {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    WriteRuntimeFrameIndexedAddressToRuntimeFrame {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    ReadRuntimeTextLine {
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_capacity: usize,
    },
    CopyRuntimeStorage {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeStorageToRuntimeFrameIndexed {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameIndexedToRuntimeFrame {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameIndexedToRuntimeStorage {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameFixedIndexedToRuntimeFrame {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameFixedIndexedToRuntimeStorage {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameFixedIndexedToRuntimePointee {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
        byte_count: usize,
    },
    /// Copy a runtime-frame slice element field (`*(frame[descriptor]) +
    /// index*elem + source_field`, index read from `frame[index_offset]`) through
    /// a `&mut` reference into its pointee field (`*(frame[pointer]) +
    /// target_field`). The runtime-index sibling of
    /// `CopyRuntimeFrameFixedIndexedToRuntimePointee` -- the `out.f = items[i].f`
    /// shape where `out` is a reference parameter.
    CopyRuntimeFrameIndexedToRuntimePointee {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        source_field_byte_offset: usize,
        pointer_byte_offset: usize,
        target_field_byte_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeMachineIndexedToRuntimeStorage {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeStorageToRuntimePointee {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_count: usize,
    },
    CopyRuntimePointeeToRuntimeFrame {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    SetDispatchState {
        dispatch_index: u32,
    },
    WriteReturnRegisterInteger {
        byte_size: usize,
        value: i64,
    },
    /// Load a runtime-storage scalar into the platform return register (w0/eax)
    /// so a NON-CONSTANT terminal value (a local read, a field read-back) becomes
    /// the process exit code, exactly like the constant path above.
    CopyRuntimeStorageToReturnRegister {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
    BeginPlatformCall,
    HostOperation {
        operation_ordinal: u16,
        operands: HandleSpan<InstructionOperand>,
    },
    PreparePlatformOutputHandle {
        capability: HostCapability,
        operands: HandleSpan<InstructionOperand>,
    },
    WritePlatformNewline {
        capability: HostCapability,
        use_file_api: bool,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

pub type SelectedInstructionKind = AbstractOperationKind;
