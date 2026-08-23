use psi_arena::Handle;

use crate::StateGuardOperator;

use super::RuntimeStorageRegion;

pub type ValueOperandHandle = Handle<ValueOperand>;
pub type AbstractValueOperandHandle = ValueOperandHandle;
pub type RuntimeValueOperandHandle = ValueOperandHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeBitFieldFragment {
    pub container_byte_offset: usize,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
}

/// The one canonical runtime value operand, shared verbatim across the
/// abstract/target/assigned operation layers (each just re-exports it under its
/// own alias). The three layers used to re-declare this enum identically, so a
/// new field/variant had to be added in three places plus the conversions; now
/// it is added here once. The per-stage arenas still differ (the conversions
/// remap `Binary` handles between them); only the TYPE is unified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueOperand {
    Immediate(i64),
    Storage {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    /// One logical scalar assembled from validated fragments inside an owned
    /// plan-laid record. The base is the containing record; every fragment
    /// offset is relative to it.
    BitField {
        region: RuntimeStorageRegion,
        base_byte_offset: usize,
        value_byte_size: usize,
        fragments: Vec<RuntimeBitFieldFragment>,
    },
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    FrameFixedIndexed {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    /// A MACHINE-owned array's runtime-indexed element read in OPERAND
    /// position (`self.k + self.arr[i]` fused into a call arg / guard /
    /// write value). The machine-region sibling of `FrameBaseIndexed`:
    /// `base_byte_offset` is relative to the MACHINE storage symbol (the
    /// operand's first relocation), and the index lives in `index_region`
    /// at `index_offset` (a frame-resident index materializes the frame
    /// base as a SECOND relocation at the architecture's pinned offset).
    /// Before this variant existed, the write selection for such fused
    /// expressions BAILED -- and the state-granular call-result fence let
    /// the dropped computation through silently (the machine-array
    /// fused-call-arg ZII).
    MachineIndexed {
        base_byte_offset: usize,
        index_region: RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    Binary {
        left: ValueOperandHandle,
        operator: StateGuardOperator,
        right: ValueOperandHandle,
        /// True when the operands are floating-point: the operation must use the
        /// SSE unit (addsd/subsd/...), not an integer add over the IEEE bits.
        is_float: bool,
        /// The resolved scalar byte width of the operation's result (4 for
        /// f32/i32, 8 for f64/i64, ...), computed ONCE at build time from the
        /// operands' scalar type. The float emission reads this to pick single
        /// (`addss`/`movss`) vs double (`addsd`/`movsd`) precision instead of
        /// re-deriving or hardcoding a width at the ISA — the
        /// scalar-width-rederivation smell. Today only the float arm consumes
        /// it; the integer arm keeps its operator/operand-derived width, so a
        /// width that cannot be resolved defaults to 8 (the prior behavior).
        byte_width: usize,
        /// The decision-17 arithmetic domain the operation must honor,
        /// resolved at build time from the operands' DECLARED types (first
        /// non-Exact witness; a domain `as` cast re-tags explicitly).
        /// Exact/Wrapping lower as the plain modular op (the byte-width
        /// compare/store truncation IS the wrap); Saturating/Trapping have NO
        /// operand-position lowering yet, so emission planning refuses them
        /// loudly -- encoding one as the plain op silently computed the
        /// unclamped wide value (150 instead of the saturated 127) or skipped
        /// the trap.
        arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain,
        /// Whether the operands are SIGNED integers, resolved from the same
        /// declared-type witness as `arithmetic_domain`. Consumed only by the
        /// Saturating/Trapping operand-position lowering (it picks the clamp
        /// bounds / overflow flag); Exact/Wrapping ignore it.
        operands_signed: bool,
    },
    /// Runtime text CONTENT equality in VALUE position (Equatable synthesis
    /// over text fields, chapter 13): each side is either a `{ptr @ +0,
    /// len @ +8}` descriptor or an owned `{len @ +0, bytes @ +8}` bounded
    /// carrier; the operand evaluates to bool 1 when the lengths match AND
    /// every byte matches, else 0. A LEAF of the operand
    /// tree (its inputs are places, not nested operands), so the AND/OR
    /// boolean tree the structural-equality expansion builds consumes it like
    /// any other compare. Lowered as a length compare plus a bounded byte
    /// loop in both ISAs (the value-position sibling of the guard-position
    /// `CompareRuntimeTextStorage` and the wire encoder's text byte copy).
    TextEquals {
        left_region: RuntimeStorageRegion,
        left_offset: usize,
        left_is_bounded_buffer: bool,
        right_region: RuntimeStorageRegion,
        right_offset: usize,
        right_is_bounded_buffer: bool,
    },
    /// Runtime text CONTENT equality against an inline string literal
    /// (`transition items[i].name == "expected"`): `place` names the String
    /// side's 16-byte `{ptr, len}` text descriptor place (a `Storage` or
    /// `FrameIndexed` operand used for its ADDRESS, never loaded as a scalar),
    /// and the literal's bytes are compared inline as immediates, so no rodata
    /// descriptor is materialized for the literal. Evaluates to bool 0/1 like
    /// `TextEquals`; a length mismatch (including an all-zero default
    /// descriptor vs a non-empty literal) is unequal without dereferencing the
    /// place's pointer.
    TextEqualsLiteral {
        place: ValueOperandHandle,
        literal: std::sync::Arc<[u8]>,
        /// The `place` is an owned `[u8; N]` bounded byte carrier (`{len, bytes}`
        /// inline) rather than a `{ptr, len}` text descriptor: the content bytes
        /// are at `place_address + pointer_size` (computed, not a stored pointer)
        /// and the length is read at `place_address + 0`. The compare is otherwise
        /// identical (length check + bounded byte loop).
        place_is_bounded_buffer: bool,
    },
    /// A numeric `as` cast applied to another operand (`(self.b as f64)` used as a
    /// binary/comparison operand): load `source`, then convert it in place
    /// (cvttsd2si / cvtsi2sd / cvtsd2ss / movsxd) to the target scalar. The result
    /// width is `target_byte_size`.
    Convert {
        source: ValueOperandHandle,
        source_byte_size: usize,
        target_byte_size: usize,
        source_is_float: bool,
        target_is_float: bool,
        source_signed: bool,
        target_signed: bool,
        /// F4: a TRAPPING float->int cast traps on NaN/out-of-range before
        /// converting; false for every other cast.
        trapping: bool,
        /// F4: a SATURATING float->int cast maps NaN to zero and clamps an
        /// out-of-range value before converting; false for every other cast.
        saturating: bool,
    },
}

/// Back-compat alias: the abstract layer's name for the shared value operand.
pub type AbstractValueOperand = ValueOperand;
/// Selection builds these directly.
pub type RuntimeValueOperand = ValueOperand;

impl Default for ValueOperand {
    fn default() -> Self {
        Self::Immediate(0)
    }
}

impl ValueOperand {
    /// Copy this operand for another stage's arena, translating the recursive
    /// `Binary` handles with `remap`. The non-recursive variants are unchanged;
    /// this exists so the per-stage conversions are a one-liner instead of a
    /// hand-written field-by-field match.
    pub fn map_handles(
        &self,
        mut remap: impl FnMut(ValueOperandHandle) -> ValueOperandHandle,
    ) -> ValueOperand {
        match self {
            Self::Binary {
                left,
                operator,
                right,
                is_float,
                byte_width,
                arithmetic_domain,
                operands_signed,
            } => Self::Binary {
                left: remap(*left),
                operator: *operator,
                right: remap(*right),
                is_float: *is_float,
                byte_width: *byte_width,
                arithmetic_domain: *arithmetic_domain,
                operands_signed: *operands_signed,
            },
            Self::TextEqualsLiteral {
                place,
                literal,
                place_is_bounded_buffer,
            } => Self::TextEqualsLiteral {
                place: remap(*place),
                literal: literal.clone(),
                place_is_bounded_buffer: *place_is_bounded_buffer,
            },
            Self::Convert {
                source,
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            } => Self::Convert {
                source: remap(*source),
                source_byte_size: *source_byte_size,
                target_byte_size: *target_byte_size,
                source_is_float: *source_is_float,
                target_is_float: *target_is_float,
                source_signed: *source_signed,
                target_signed: *target_signed,
                trapping: *trapping,
                saturating: *saturating,
            },
            other => other.clone(),
        }
    }
}
