use omega_core::arena::Handle;

use crate::StateGuardOperator;

use super::RuntimeStorageRegion;

pub type AbstractValueOperandHandle = Handle<AbstractValueOperand>;
pub type RuntimeValueOperandHandle = AbstractValueOperandHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractValueOperand {
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
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
        /// True when the operands are floating-point: the operation must use the
        /// SSE unit (addsd/subsd/...), not an integer add over the IEEE bits.
        is_float: bool,
    },
}

pub type RuntimeValueOperand = AbstractValueOperand;

impl Default for AbstractValueOperand {
    fn default() -> Self {
        Self::Immediate(0)
    }
}
