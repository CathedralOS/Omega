use crate::{RuntimeStorageRegion, StateGuardOperator};
use omega_core::arena::Handle;

pub type TargetValueOperandHandle = Handle<TargetValueOperand>;
pub type RuntimeValueOperandHandle = TargetValueOperandHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetValueOperand {
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
    Binary {
        left: TargetValueOperandHandle,
        operator: StateGuardOperator,
        right: TargetValueOperandHandle,
    },
}

pub type RuntimeValueOperand = TargetValueOperand;

impl Default for TargetValueOperand {
    fn default() -> Self {
        Self::Immediate(0)
    }
}
