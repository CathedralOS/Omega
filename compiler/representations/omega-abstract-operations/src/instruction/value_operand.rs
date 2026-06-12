use omega_core::arena::Handle;

use crate::StateGuardOperator;

use super::RuntimeStorageRegion;

pub type ValueOperandHandle = Handle<ValueOperand>;
pub type AbstractValueOperandHandle = ValueOperandHandle;
pub type RuntimeValueOperandHandle = ValueOperandHandle;

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
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_offset: usize,
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
    Binary {
        left: ValueOperandHandle,
        operator: StateGuardOperator,
        right: ValueOperandHandle,
        /// True when the operands are floating-point: the operation must use the
        /// SSE unit (addsd/subsd/...), not an integer add over the IEEE bits.
        is_float: bool,
    },
    /// Runtime text CONTENT equality in VALUE position (Equatable synthesis
    /// over `String` fields, chapter 13): both sides are `{ptr @ +0, len @ +8}`
    /// text descriptor places; the operand evaluates to bool 1 when the
    /// lengths match AND every byte matches, else 0. A LEAF of the operand
    /// tree (its inputs are places, not nested operands), so the AND/OR
    /// boolean tree the structural-equality expansion builds consumes it like
    /// any other compare. Lowered as a length compare plus a bounded byte
    /// loop in both ISAs (the value-position sibling of the guard-position
    /// `CompareRuntimeTextStorage` and the wire encoder's text byte copy).
    TextEquals {
        left_region: RuntimeStorageRegion,
        left_offset: usize,
        right_region: RuntimeStorageRegion,
        right_offset: usize,
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
        literal: String,
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
            } => Self::Binary {
                left: remap(*left),
                operator: *operator,
                right: remap(*right),
                is_float: *is_float,
            },
            Self::TextEqualsLiteral { place, literal } => Self::TextEqualsLiteral {
                place: remap(*place),
                literal: literal.clone(),
            },
            Self::Convert {
                source,
                source_byte_size,
                target_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
            } => Self::Convert {
                source: remap(*source),
                source_byte_size: *source_byte_size,
                target_byte_size: *target_byte_size,
                source_is_float: *source_is_float,
                target_is_float: *target_is_float,
                source_signed: *source_signed,
            },
            other => other.clone(),
        }
    }
}
