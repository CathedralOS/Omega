use crate::AssignedValueHomeKind;
use omega_target_operations::{RuntimeStorageRegion, StateGuardOperator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedValueOperand {
    pub kind: AssignedValueOperandKind,
    pub home: AssignedValueHomeKind,
}

impl Default for AssignedValueOperand {
    fn default() -> Self {
        Self {
            kind: AssignedValueOperandKind::Immediate(0),
            home: AssignedValueHomeKind::Immediate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedValueOperandKind {
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
        left: AssignedValueOperandHandle,
        operator: StateGuardOperator,
        right: AssignedValueOperandHandle,
    },
}

impl From<omega_target_operations::TargetValueOperand> for AssignedValueOperandKind {
    fn from(kind: omega_target_operations::TargetValueOperand) -> Self {
        match kind {
            omega_target_operations::TargetValueOperand::Immediate(value) => Self::Immediate(value),
            omega_target_operations::TargetValueOperand::Storage {
                region,
                byte_offset,
                byte_size,
            } => Self::Storage {
                region,
                byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Self::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::Binary {
                left,
                operator,
                right,
            } => Self::Binary {
                left,
                operator,
                right,
            },
        }
    }
}

impl From<AssignedValueOperandKind> for omega_target_operations::TargetValueOperand {
    fn from(kind: AssignedValueOperandKind) -> Self {
        match kind {
            AssignedValueOperandKind::Immediate(value) => Self::Immediate(value),
            AssignedValueOperandKind::Storage {
                region,
                byte_offset,
                byte_size,
            } => Self::Storage {
                region,
                byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Self::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameBaseIndexed {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::Binary {
                left,
                operator,
                right,
            } => Self::Binary {
                left,
                operator,
                right,
            },
        }
    }
}

pub type AssignedValueOperandHandle = omega_target_operations::TargetValueOperandHandle;
pub type RuntimeValueOperand = AssignedValueOperandKind;
pub type RuntimeValueOperandHandle = AssignedValueOperandHandle;
pub type TargetValueOperand = AssignedValueOperandKind;
pub type TargetValueOperandHandle = AssignedValueOperandHandle;

pub(crate) fn assigned_value_handle(
    handle: RuntimeValueOperandHandle,
) -> omega_core::arena::Handle<AssignedValueOperand> {
    omega_core::arena::Handle::from_parts(handle.arena_index(), handle.generation())
}

pub(crate) fn target_value_handle(
    handle: omega_core::arena::Handle<AssignedValueOperand>,
) -> RuntimeValueOperandHandle {
    omega_core::arena::Handle::from_parts(handle.arena_index(), handle.generation())
}
