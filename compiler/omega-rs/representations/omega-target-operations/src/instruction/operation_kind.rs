use crate::{
    HostOperationKey, InstructionOperand, Place, RuntimeStorageRegion, RuntimeTextReadSource,
    StateGuardLowering, StateGuardOperator, TargetDataObjectHandle, TargetValueOperandHandle,
};
use omega_core::arena::HandleSpan;

mod classification;
#[cfg(test)]
mod tests;

pub use classification::TargetOperationDomain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOperationKind {
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
        is_float: bool,
    },
    CompareRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        literal: std::sync::Arc<str>,
    },
    CompareRuntimeTextStorage {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeValues {
        left: TargetValueOperandHandle,
        right: TargetValueOperandHandle,
        byte_size: usize,
        operator: StateGuardOperator,
    },

    /// Task #131: the place-shaped storage compare (guards consume Places).
    ComparePlaces {
        left: Place,
        right: Place,
        byte_size: usize,
        operator: StateGuardOperator,
        is_float: bool,
    },

    /// Task #131: the place-vs-immediate compare.
    ComparePlaceValue {
        place: Place,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        literal: std::sync::Arc<str>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: TargetDataObjectHandle,
        byte_offset: usize,
        literal: std::sync::Arc<str>,
    },
    AppendRuntimeTextStoredSuffix {
        buffer: TargetDataObjectHandle,
        buffer_offset: usize,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        length_delta: usize,
    },

    /// Task #132: the place-shaped text-buffer materialize (the 3-shape
    /// Materialize crossing's survivor).
    MaterializeTextBufferToPlace {
        buffer: TargetDataObjectHandle,
        target: Place,
    },

    /// Task #132: the place-shaped stored-text append.
    AppendTextStoredToPlace {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target: Place,
    },

    /// Task #132: the place-shaped literal append.
    AppendTextLiteralToPlace {
        buffer: TargetDataObjectHandle,
        target: Place,
        literal: std::sync::Arc<str>,
    },
    /// compact_binary v0 wire framing: store one compile-time byte into the
    /// encode buffer at the stored cursor (the caller's `written` slot), then
    /// advance the cursor by one.
    AppendWireLiteralByte {
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
        value: u8,
    },
    /// compact_binary v0 wire framing: LEB128-encode a runtime scalar
    /// (zigzagged first when `zigzag` is set) into the encode buffer at the
    /// stored cursor, advancing the cursor by the encoded byte count.
    AppendWireScalarVarint {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        byte_size: usize,
        zigzag: bool,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    /// compact_binary v0 wire framing: append a runtime `String` field (a
    /// `{ptr @ +0, len @ +8}` text descriptor at the source place) as the len
    /// LEB128 varint followed by len raw bytes copied from ptr; every
    /// byte-copy store bounds against `out_length` and drops content past
    /// capacity.
    AppendWireTextBytes {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        out_region: RuntimeStorageRegion,
        out_offset: usize,
        out_length: usize,
        written_region: RuntimeStorageRegion,
        written_offset: usize,
    },
    /// compact_binary v0 wire decoding: expect one compile-time framing byte
    /// at the stored cursor (the caller's `read` slot); a bounds miss or a
    /// mismatch clears the sticky `ok` flag.
    ReadWireExpectedByte {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        expected: u8,
    },
    /// compact_binary v0 wire decoding: LEB128-read a runtime scalar at the
    /// stored cursor into the target place (un-zigzagging when `zigzag` is
    /// set); truncated or overlong varints clear the sticky `ok` flag.
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
        byte_size: usize,
        zigzag: bool,
    },
    /// compact_binary v0 borrowed `&[u8]` decode (#43): read a byte-length
    /// varint, bounds-check it, store `{ptr = &buffer[cursor], len}` into the
    /// target descriptor, advance the cursor past the content.
    ReadWireByteSlice {
        buffer_region: RuntimeStorageRegion,
        buffer_offset: usize,
        buffer_length: usize,
        read_region: RuntimeStorageRegion,
        read_offset: usize,
        ok_region: RuntimeStorageRegion,
        ok_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        /// Decode-boundary byte-domain obligations
        /// (`ByteSequencePredicate::mask_bit` bits): the emitted sequence
        /// validates the copied bytes and clears the sticky `ok` flag on
        /// failure. ZII: an empty mask is a plain byte copy.
        predicate_mask: u8,
    },
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
    AppendWireRepeatedScalarVarint {
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
        target_offset: usize,
        byte_size: usize,
        zigzag: bool,
    },
    /// Entry prologue: store the normalized plan's incoming argument register
    /// into the entry parameter's frame slot.
    WriteEntryArgumentRegister {
        register: omega_calling_conventions::MachineRegister,
        byte_offset: usize,
        byte_size: usize,
    },
    /// Entry prologue: bind `args: &[u8]` as {ptr -> frame+spill, len}.
    WriteEntryArgumentsSliceDescriptor {
        descriptor_offset: usize,
        spill_offset: usize,
        byte_length: usize,
    },
    WriteRuntimeStorageConvert {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        target_byte_size: usize,
        source: TargetValueOperandHandle,
        source_byte_size: usize,
        source_is_float: bool,
        target_is_float: bool,
        source_signed: bool,
        /// Whether the integer target is signed (float->int policy/conversion).
        target_signed: bool,
        /// F4: a TRAPPING float->int cast traps on NaN/out-of-range before
        /// converting; false for every other cast.
        trapping: bool,
        /// F4: a SATURATING float->int cast maps NaN to zero and clamps an
        /// out-of-range value to the target bounds. Exact casts leave
        /// this false because their range obligation was already discharged.
        saturating: bool,
    },
    /// Atomic `fetch_add`: `LOCK xadd` of `delta` into the storage place.
    AtomicFetchAdd {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        delta: TargetValueOperandHandle,
    },
    /// Atomic `compare_exchange`: `LOCK CMPXCHG` (x86) / `CASAL` (aarch64) of the
    /// storage place against `expected`, swapping in `new_value` only on match.
    AtomicCompareExchange {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        expected: TargetValueOperandHandle,
        new_value: TargetValueOperandHandle,
    },
    /// Append a source carrier's content onto a target carrier (concat builder
    /// source segment). See the abstract-operations twin.
    AppendRuntimeMachineBoundedBufferSource {
        target_byte_offset: usize,
        source_byte_offset: usize,
        source_in_frame: bool,
    },
    /// Append a string LITERAL onto an owned `[u8; N]` carrier at its running
    /// length (a later concat segment such as the trailing `" =="`). See the
    /// abstract-operations twin.
    AppendRuntimeMachineBoundedBufferLiteral {
        target_byte_offset: usize,
        literal: std::sync::Arc<str>,
    },
    ReadRuntimeTextLine {
        buffer: TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_capacity: usize,
        source: RuntimeTextReadSource,
        /// The target is an owned `[u8; N]` carrier (`{len, bytes}` inline): read
        /// stdin into its inline bytes (`region + target_offset + pointer_size`)
        /// and write only `len` at `target_offset`; `buffer` is unused.
        is_bounded_buffer: bool,
    },
    /// One stdin byte into a `ByteRead` sum slot (see the abstract kind's
    /// doc: ZII-driven, no scratch object -- zero tag + payload, read into
    /// the payload word, tag 1 only on count > 0; the zero state IS `Eof`).
    ReadRuntimeByte {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        payload_offset: usize,
        source: RuntimeTextReadSource,
    },
    /// One byte to stdout straight from the argument's storage
    /// (little-endian low byte first; a literal rides a 1-byte data object
    /// with `source_is_place` false).
    WriteRuntimeByte {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        literal: TargetDataObjectHandle,
        source_is_place: bool,
        source: RuntimeTextReadSource,
    },
    /// The Place-pair copy (codegen cleanup Phase 6, rung 2): addressing
    /// lives in the two [`Place`] operands -- base region + composable
    /// steps -- not in the variant name. The relocation walker patches each
    /// base-materialization site from the corresponding place's own region;
    /// the per-variant Copy* kinds retire into this one as selection
    /// migrates (`CopyRuntimeStorage` was the first, retired 2026-07-14).
    CopyPlaces {
        source: Place,
        target: Place,
        byte_count: usize,
    },

    /// Write rung 2a: store an immediate integer at `byte_size` into a
    /// place-shaped target -- the integer-write family's collapse (the
    /// seven Write*Integer variants migrate onto this one).
    WritePlaceInteger {
        target: Place,
        value: i64,
        byte_size: usize,
    },

    /// Binary rung 2a: `place = left OP right` -- the six Write*Binary
    /// variants collapse onto this one. Field semantics mirror the retired
    /// storage-binary write (is_float = SSE unit; domain = the target
    /// type's arithmetic domain, decision 17; target_signed = OF-vs-CF
    /// overflow detection + saturating clamp bounds).
    WritePlaceBinary {
        target: Place,
        byte_size: usize,
        left: TargetValueOperandHandle,
        operator: StateGuardOperator,
        right: TargetValueOperandHandle,
        is_float: bool,
        domain: omega_core::arithmetic::ArithmeticDomain,
        target_signed: bool,
    },

    /// Text rung 2a: the place-shaped string-descriptor write (the five
    /// Write*String variants' survivor).
    WritePlaceString {
        target: Place,
        data: TargetDataObjectHandle,
        byte_length: usize,
    },

    /// Text rung 2a: the place-shaped bounded-buffer literal write (the two
    /// *BoundedBuffer write variants' survivor); content is immediate.
    WritePlaceBoundedBuffer {
        target: Place,
        literal: std::sync::Arc<str>,
    },

    /// Task #131: the place-shaped address write (the six
    /// Write*AddressToRuntimeFrame variants' survivor).
    WritePlaceAddress {
        source: Place,
        target_offset: usize,
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
        operation_key: HostOperationKey,
        operands: HandleSpan<InstructionOperand>,
    },
    /// The x86 `hlt` privileged instruction (`asm { hlt }`), emitting the
    /// `machine_control` effect. Zero operands, no relocation. See the
    /// privileged_effects_and_binary_trust brief.
    MachineHalt,
    /// An x86 load/store/full memory-ordering fence.
    MemoryFence(omega_core::inline_assembly::AsmFenceKind),
    /// x86 CLI/STI interrupt-flag control.
    InterruptControl(omega_core::inline_assembly::AsmInterruptControlKind),
    /// Compiler-balanced `pushfq` snapshot into explicit runtime storage.
    FlagsSnapshot {
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    /// Compiler-balanced `popfq` restore from an explicit u64 operand.
    FlagsRestore {
        source: TargetValueOperandHandle,
    },
    /// Structured x86 RDMSR: u32 index operand, u64 destination place.
    MsrRead {
        index: TargetValueOperandHandle,
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    /// Structured x86 WRMSR: u32 index plus u64 value operands.
    MsrWrite {
        index: TargetValueOperandHandle,
        value: TargetValueOperandHandle,
    },
    ControlRegisterRead {
        register: omega_core::inline_assembly::AsmControlRegister,
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    ControlRegisterWrite {
        register: omega_core::inline_assembly::AsmControlRegister,
        source: TargetValueOperandHandle,
    },
    /// The x86 `out dx, al` port write (`asm { out <port>, <value> }`),
    /// emitting `device_io`. `port` u16 + `value` u8 operands (immediate or
    /// storage; storage operands relocate).
    PortWrite {
        port: TargetValueOperandHandle,
        value: TargetValueOperandHandle,
    },
    /// The x86 `in al, dx` port read (`asm { in <dest>, <port> }`), emitting
    /// `device_io`. `port` u16 operand; the byte result stores to the
    /// destination place.
    PortRead {
        port: TargetValueOperandHandle,
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    LeaveFunction,
}

pub type SelectedInstructionKind = TargetOperationKind;
