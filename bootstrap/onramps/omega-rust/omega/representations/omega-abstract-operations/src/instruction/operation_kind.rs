use crate::{
    AbstractDataObjectHandle, AbstractValueOperandHandle, InstructionOperand, Place,
    RuntimeStorageRegion, RuntimeTextReadTarget, StateGuardLowering, StateGuardOperator,
};
use omega_calling_conventions::HostCapability;
use psi_arena::HandleSpan;
use std::sync::Arc;

mod classification;
#[cfg(test)]
mod tests;

pub use classification::AbstractOperationDomain;

/// Semantic provenance retained on the canonical place-pair copy. Boundary
/// copies remain the same operation and byte program as ordinary copies, but
/// final footprint validation must not infer ABI evidence from shape alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CopyPlacesRole {
    #[default]
    Ordinary,
    ExitIndirectResult,
}

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
        literal: Arc<[u8]>,
    },
    CompareRuntimeTextStorage {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeValues {
        left: AbstractValueOperandHandle,
        right: AbstractValueOperandHandle,
        byte_size: usize,
        operator: StateGuardOperator,
    },

    /// Task #131 (guards consume Places): compare two place-shaped storage
    /// operands. Direct places are the retired storage compare; indexed /
    /// deref places ride the materializer walk (a two-index RIGHT place
    /// refuses at encoding -- the register fence).
    ComparePlaces {
        left: Place,
        right: Place,
        byte_size: usize,
        operator: StateGuardOperator,
        /// Both operands are f64 and the compare uses `ucomisd` (whose
        /// CF/ZF mirror an unsigned `cmp`).
        is_float: bool,
    },

    /// Task #131: compare a place-shaped storage operand against an
    /// immediate.
    ComparePlaceValue {
        place: Place,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: AbstractDataObjectHandle,
        literal: Arc<[u8]>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: AbstractDataObjectHandle,
        byte_offset: usize,
        literal: Arc<[u8]>,
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

    /// Task #132 (op-set shrink): materialize a text-buffer descriptor into
    /// a place-shaped target -- the former shape crossing collapses onto this
    /// one. X86 uses its general place materializer; AArch64 decomposes into
    /// the retained classified encoders.
    MaterializeTextBufferToPlace {
        buffer: AbstractDataObjectHandle,
        target: Place,
    },

    /// Task #132: append a stored text value onto the builder buffer and
    /// store the descriptor into a place-shaped target (the AppendStored
    /// crossing's survivor).
    AppendTextStoredToPlace {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target: Place,
    },

    /// Task #132: append a literal onto the builder buffer and store the
    /// descriptor into a place-shaped target (the AppendLiteral crossing's
    /// survivor).
    AppendTextLiteralToPlace {
        buffer: AbstractDataObjectHandle,
        target: Place,
        literal: Arc<[u8]>,
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
    /// compact_binary v0 borrowed scalar-slice encoding: the source is a
    /// `{ptr, element_count}` slice descriptor. The operation measures the
    /// exact packed-varint body, verifies that the remaining output capacity
    /// covers its canonical length prefix plus body, then walks the elements
    /// again to emit them. This is the executable form of the normalized
    /// runtime length/work/output-capacity obligation.
    AppendWireScalarSlice {
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
    /// target is stored after syntactically complete decodes; malformed
    /// varints may leave unspecified field contents. A complete value that
    /// violates `range` instead preserves the prior field value while
    /// clearing the sticky `ok` flag.
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
        /// Inclusive destination range established from hostile bytes before
        /// the constrained field is written. Out-of-range input clears `ok`
        /// and leaves the prior target value untouched.
        range: Option<psi_language_semantics::wire::WireScalarRange>,
    },
    /// compact_binary v0 wire decoding (#43, borrowed `&[u8]` fields): read a
    /// byte-LENGTH varint, bounds-check it, then store a fat `{ptr, len}`
    /// descriptor VIEWING the buffer -- `ptr = &buffer[cursor]` (the content,
    /// just past the length varint) and `len` = the decoded length -- into the
    /// target descriptor slot, and advance the cursor past the content. A length
    /// running past the buffer clears the sticky `ok` flag (the cursor stops at
    /// the buffer end). Zero-copy: the decoded `&[u8]` borrows the decode buffer.
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
    /// (the runtime element count in the FixedVec carrier's `length` slot,
    /// read as an unsigned 64-bit count). A repeated field's element count is
    /// runtime-sized but bounded by the carrier's static capacity, so
    /// selection unrolls the capacity and guards each append -- the emitted
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
    /// FixedVec `length` slot. Selection unrolls the declared capacity, so a
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
        /// Inclusive range established for each decoded destination element.
        /// Out-of-range input clears `ok` and preserves that element's prior
        /// valid value.
        range: Option<psi_language_semantics::wire::WireScalarRange>,
    },
    /// The ENTRY PROLOGUE's inbound calling plan: store the platform's incoming
    /// argument register selected by the normalized boundary `CallPlan` into
    /// the entry state's parameter frame slot at `byte_offset`. Emitted once per declared entry
    /// parameter, BEFORE anything else at the entry (registers are volatile) --
    /// this is how a UEFI `main(image_handle, system_table)` receives the
    /// firmware handoff (calling_plans.md, the entry-stub inbound direction).
    WriteEntryArgumentRegister {
        register: omega_calling_conventions::MachineRegister,
        byte_offset: usize,
        byte_size: usize,
    },
    /// Entry prologue: copy one normalized incoming stack-argument fragment
    /// into the entry parameter's runtime-frame slot. `stack_byte_offset` is
    /// relative to the ABI's incoming stack-argument area; the target encoder
    /// accounts for its return address and/or function-enter frame.
    WriteEntryStackArgument {
        stack_byte_offset: u32,
        byte_offset: usize,
        byte_size: usize,
    },
    /// Entry prologue: copy a complete indirectly passed aggregate from the
    /// pointer location selected by the normalized calling plan into its
    /// runtime-frame parameter slot.
    WriteEntryIndirectArgument {
        pointer: omega_calling_conventions::IndirectPointerLocation,
        byte_offset: usize,
        byte_size: usize,
    },
    /// The bytes-handoff half of the entry prologue: bind the entry's
    /// `args: &[u8]` parameter as a view over the ENTRY-ARGUMENT SPILL (where
    /// the prologue stored the platform's argument registers). Writes the
    /// 16-byte slice descriptor {ptr @ +0 -> frame+spill_offset, len @ +8}.
    WriteEntryArgumentsSliceDescriptor {
        descriptor_offset: usize,
        spill_offset: usize,
        byte_length: usize,
    },
    /// Atomic load from a direct storage place into a direct result place.
    AtomicLoad {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// Atomic store of one computed operand into a direct storage place.
    AtomicStore {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        value: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// An atomic `fetch_add`: atomically add `delta` to the storage place via a
    /// single target RMW. The instruction-observed prior value is written to
    /// `result_region + result_offset`; no separate ordinary read is legal.
    AtomicFetchAdd {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        delta: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// An atomic `fetch_sub`: atomically subtract `delta` from the storage
    /// place while returning the instruction-observed prior value.
    AtomicFetchSub {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        delta: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// An atomic `fetch_xor`: atomically XOR `value` into the storage place
    /// while returning the instruction-observed prior value.
    AtomicFetchXor {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        value: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// An atomic `fetch_or`: atomically OR `value` into the storage place
    /// while returning the instruction-observed prior value.
    AtomicFetchOr {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        value: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// An atomic `fetch_and`: atomically AND `value` into the storage place
    /// while returning the instruction-observed prior value.
    AtomicFetchAnd {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        value: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// Atomically replace the storage value and return the instruction-observed
    /// prior value through a distinct result place.
    AtomicSwap {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        new_value: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
    },
    /// An atomic `compare_exchange`: atomically compare the storage place against
    /// `expected` and, only if equal, swap in `new_value`, via a single `LOCK
    /// CMPXCHG` (x86) / `CAS*` (aarch64). The instruction-observed prior value
    /// is written to `result_region + result_offset`.
    AtomicCompareExchange {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        result_region: RuntimeStorageRegion,
        result_offset: usize,
        expected: AbstractValueOperandHandle,
        new_value: AbstractValueOperandHandle,
        ordering: psi_language_core::AtomicOrderingPlan,
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
        /// Whether the integer target is signed. Float->int policy bounds and
        /// the selected conversion instruction depend on this independently
        /// of the source's signedness.
        target_signed: bool,
        /// F4: a TRAPPING float->int cast traps on NaN or an out-of-range
        /// value BEFORE converting (the encoders emit the FP bound guard).
        /// False for every other cast.
        trapping: bool,
        /// F4: a SATURATING float->int cast maps NaN to zero and clamps an
        /// out-of-range value to the target bounds. Exact casts leave
        /// this false because their range obligation was already discharged.
        saturating: bool,
    },
    /// A numeric `as` cast stored through a composed place (for example a
    /// runtime-indexed array element). The conversion contract is identical to
    /// [`Self::WriteRuntimeStorageConvert`]; only target addressing differs.
    WritePlaceConvert {
        target: Place,
        target_byte_size: usize,
        source: AbstractValueOperandHandle,
        source_byte_size: usize,
        source_is_float: bool,
        target_is_float: bool,
        source_signed: bool,
        target_signed: bool,
        trapping: bool,
        saturating: bool,
    },
    /// Append another owned `[u8; N]` carrier's content onto a target carrier.
    /// Both addresses ride the ordinary [`Place`] algebra, so a concat has one
    /// semantic operation whether either carrier is direct, borrowed through a
    /// parameter, or eventually indexed. The length-fits guard has already
    /// proved that the result fits the target's `N`.
    AppendPlaceBoundedBufferSource {
        target: Place,
        source: Place,
    },
    /// Append a string LITERAL onto an owned `[u8; N]` carrier at its running
    /// length (a later concat segment, e.g. the trailing `" =="` of
    /// `"== " + room.label + " =="`). The literal's bytes are written as immediates
    /// at `target + pointer_size + len`, then `len += literal.len`. The length-fits
    /// guard proves the result still fits the target's `N`.
    AppendPlaceBoundedBufferLiteral {
        target: Place,
        literal: Arc<[u8]>,
    },
    ReadRuntimeTextLine {
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_capacity: usize,
        /// Storage representation receiving the bytes. `buffer` is used only
        /// for [`RuntimeTextReadTarget::StringDescriptor`].
        target: RuntimeTextReadTarget,
    },
    /// One stdin byte into a `ByteRead` sum slot (std console `read_byte()`,
    /// no scratch object -- ZII-driven): zero the tag word AND the payload
    /// word, `read(fd 0, target + payload_offset, 1)` lands the byte straight
    /// in the pre-zeroed payload (little-endian low byte), and ONLY a
    /// count > 0 read writes tag 1 (`Byte`) -- the untouched zero state IS
    /// `Eof` (ordinal 0). The tag sits at `target_offset` (ENUM_TAG_BYTES
    /// wide); the payload word at `target_offset + payload_offset`.
    ReadRuntimeByte {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        payload_offset: usize,
    },
    /// One byte to stdout (std console `write_byte(b)`): `write(fd 1,
    /// source, 1)` straight from the argument's storage -- on little-endian
    /// the first byte of an integer place IS its low byte, so no staging
    /// copy exists. A literal argument rides a 1-byte data object instead
    /// (`source_is_place` false; `literal` holds the byte, the
    /// FirstTextArgument literal precedent).
    WriteRuntimeByte {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        literal: AbstractDataObjectHandle,
        source_is_place: bool,
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
        role: CopyPlacesRole,
    },

    /// Write rung 2a: store an immediate integer at `byte_size` into a
    /// place-shaped target -- the integer-write family's collapse (the
    /// seven Write*Integer variants migrate onto this one).
    WritePlaceInteger {
        target: Place,
        value: i64,
        byte_size: usize,
    },
    /// Store one immediate logical scalar through a validated fragmented
    /// plan-laid field. Each destination container is updated by masked RMW;
    /// fragments are complete source tiling established by layout validation.
    WriteStorageBitField {
        region: RuntimeStorageRegion,
        base_byte_offset: usize,
        fragments: Vec<crate::RuntimeBitFieldFragment>,
        value: i64,
    },

    /// Binary rung 2a: `place = left OP right` -- the six Write*Binary
    /// variants collapse onto this one. Field semantics mirror the retired
    /// storage-binary write (is_float = SSE unit; domain = the target
    /// type's arithmetic domain, decision 17; target_signed = OF-vs-CF
    /// overflow detection + saturating clamp bounds).
    WritePlaceBinary {
        target: Place,
        byte_size: usize,
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
        is_float: bool,
        domain: psi_numerics::arithmetic::ArithmeticDomain,
        target_signed: bool,
    },

    /// Text rung 2a: store a string DESCRIPTOR ({ptr -> rodata, len}) into a
    /// place-shaped target -- the five Write*String variants collapse onto
    /// this one. The data pointer's relocation rides the leading
    /// materialization (instruction start on x86_64).
    WritePlaceString {
        target: Place,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },

    /// Text rung 2a: write a string literal into an owned `[u8; N]` bounded
    /// byte carrier ({len, bytes} inline) at a place-shaped target -- the
    /// two *BoundedBuffer write variants collapse onto this one. The content
    /// is immediate, so the walk's base relocation(s) are the only sites.
    WritePlaceBoundedBuffer {
        target: Place,
        literal: Arc<[u8]>,
    },

    /// Task #131: store the ADDRESS of a place-shaped source into the
    /// runtime-frame slot at `target_offset` -- the six
    /// Write*AddressToRuntimeFrame variants collapse onto this one. The
    /// slot receives a REAL POINTER; reads deref it.
    WritePlaceAddress {
        source: Place,
        target_offset: usize,
    },
    SetDispatchState {
        dispatch_index: u32,
    },
    WriteReturnRegisterInteger {
        /// Exact integer result register selected by the normalized call plan.
        register: omega_calling_conventions::MachineRegister,
        byte_size: usize,
        value: i64,
    },
    /// Load a runtime-storage scalar into the platform return register (w0/eax)
    /// so a NON-CONSTANT terminal value (a local read, a field read-back) becomes
    /// the process exit code, exactly like the constant path above.
    CopyRuntimeStorageToReturnRegister {
        /// Exact integer result register selected by the normalized call plan.
        register: omega_calling_conventions::MachineRegister,
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
    /// Compiler-private direct call to one exact lowered function identity.
    /// Argument/receiver placement is owned by a separate validated call plan;
    /// this operation only owns the control-transfer edge.
    CallInternalFunction {
        target: omega_control_flow::MachineFunctionIdentity,
    },
    /// Compiler-private caller-frame recipe: compute one address at a positive
    /// displacement from the current RSP into an ABI argument register. This
    /// does not reserve or write stack storage and does not perform a call.
    LoadOutgoingStackAddress {
        register: omega_calling_conventions::MachineRegister,
        stack_byte_offset: u32,
    },
    ReserveOutgoingStackFrame {
        byte_count: u32,
    },
    WriteOutgoingStackU64 {
        stack_byte_offset: u32,
        value: u64,
    },
    /// Copy one launch-time u64 field through an incoming indirect argument
    /// pointer into the live compiler-private outgoing caller frame.
    CopyEntryIndirectU64ToOutgoingStack {
        source_register: omega_calling_conventions::MachineRegister,
        source_byte_offset: u32,
        stack_byte_offset: u32,
    },
    ReleaseOutgoingStackFrame {
        byte_count: u32,
    },
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
    /// The x86 `hlt` privileged instruction (`asm { hlt }`), emitting the
    /// `MachineControl` service. Zero operands, no relocation: it idles the
    /// CPU until the next interrupt. Only reachable in a freestanding boundary
    /// root (v0 discharge). See privileged_effects_and_binary_trust brief.
    MachineHalt,
    /// An x86 load/store/full memory-ordering fence. Zero operands and no
    /// relocations; the kind selects the exact opcode at emission.
    MemoryFence(psi_language_core::inline_assembly::AsmFenceKind),
    /// x86 CLI/STI interrupt-flag control.
    InterruptControl(psi_language_core::inline_assembly::AsmInterruptControlKind),
    /// Compiler-balanced `pushfq` snapshot into explicit runtime storage.
    FlagsSnapshot {
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    /// Compiler-balanced `popfq` restore from an explicit u64 operand.
    FlagsRestore {
        source: AbstractValueOperandHandle,
    },
    /// Structured x86 RDMSR: u32 index operand, u64 destination place.
    MsrRead {
        index: AbstractValueOperandHandle,
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    /// Structured x86 WRMSR: u32 index plus u64 value operands.
    MsrWrite {
        index: AbstractValueOperandHandle,
        value: AbstractValueOperandHandle,
    },
    ControlRegisterRead {
        register: psi_language_core::inline_assembly::AsmControlRegister,
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    ControlRegisterWrite {
        register: psi_language_core::inline_assembly::AsmControlRegister,
        source: AbstractValueOperandHandle,
    },
    /// The x86 `out dx, al` port write (`asm { out <port>, <value> }`),
    /// reaching the `PortIo` service. `port` is a u16 operand, `value` a u8
    /// operand (each an immediate or a storage read; storage operands relocate
    /// like any runtime-value read).
    PortWrite {
        port: AbstractValueOperandHandle,
        value: AbstractValueOperandHandle,
    },
    /// The x86 `in al, dx` port read (`asm { in <dest>, <port> }`), emitting
    /// the `PortIo` service. `port` is a u16 operand; the byte result is
    /// stored to the destination place (`dest_region`/`dest_byte_offset`).
    PortRead {
        port: AbstractValueOperandHandle,
        dest_region: RuntimeStorageRegion,
        dest_byte_offset: usize,
    },
    LeaveFunction,
}

pub type SelectedInstructionKind = AbstractOperationKind;
