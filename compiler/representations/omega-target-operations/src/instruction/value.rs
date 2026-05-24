use crate::{RuntimeStorageRegion, StateGuardOperator};
use omega_core::arena::{Arena, Handle};

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

pub trait RuntimeValueOperandSource {
    fn immediate_integer(&self, handle: RuntimeValueOperandHandle) -> Option<i64>;
    fn storage(&self, handle: RuntimeValueOperandHandle)
        -> Option<(RuntimeStorageRegion, usize, usize)>;
    fn pointee(&self, handle: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)>;
    fn frame_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)>;
    fn binary(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, StateGuardOperator, RuntimeValueOperandHandle)>;
}

impl RuntimeValueOperandSource for Arena<RuntimeValueOperand> {
    fn immediate_integer(&self, handle: RuntimeValueOperandHandle) -> Option<i64> {
        match self.get(handle) {
            RuntimeValueOperand::Immediate(value) => Some(*value),
            _ => None,
        }
    }

    fn storage(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::Storage {
                region,
                byte_offset,
                byte_size,
            } => Some((*region, *byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn pointee(&self, handle: RuntimeValueOperandHandle) -> Option<(usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Some((*pointer_byte_offset, *field_byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn frame_indexed(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match self.get(handle) {
            RuntimeValueOperand::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *descriptor_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
                *byte_size,
            )),
            _ => None,
        }
    }

    fn binary(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<(RuntimeValueOperandHandle, StateGuardOperator, RuntimeValueOperandHandle)> {
        match self.get(handle) {
            RuntimeValueOperand::Binary {
                left,
                operator,
                right,
            } => Some((*left, *operator, *right)),
            _ => None,
        }
    }
}

impl Default for TargetValueOperand {
    fn default() -> Self {
        Self::Immediate(0)
    }
}
