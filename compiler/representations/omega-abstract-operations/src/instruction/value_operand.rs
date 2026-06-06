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
            other => other.clone(),
        }
    }
}
