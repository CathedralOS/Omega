use crate::AssignedValueHomeKind;
use omega_target_operations::{RuntimeStorageRegion, StateGuardOperator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedInstructionOperand {
    pub kind: AssignedInstructionOperandKind,
}

impl Default for AssignedInstructionOperand {
    fn default() -> Self {
        Self {
            kind: AssignedInstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedInstructionOperandKind {
    DataAddress {
        data: omega_target_operations::TargetDataObjectHandle,
    },
    RuntimeStringPointer {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    RuntimeStringLength {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    ImmediateInteger(i64),
    ByteLength(usize),
}

impl From<omega_target_operations::TargetInstructionOperandKind>
    for AssignedInstructionOperandKind
{
    fn from(kind: omega_target_operations::TargetInstructionOperandKind) -> Self {
        match kind {
            omega_target_operations::TargetInstructionOperandKind::DataAddress { data } => {
                Self::DataAddress { data }
            }
            omega_target_operations::TargetInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimeStringPointer {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Self::RuntimeStringLength {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::ImmediateInteger(value) => {
                Self::ImmediateInteger(value)
            }
            omega_target_operations::TargetInstructionOperandKind::ByteLength(value) => {
                Self::ByteLength(value)
            }
        }
    }
}

impl From<AssignedInstructionOperandKind>
    for omega_target_operations::TargetInstructionOperandKind
{
    fn from(kind: AssignedInstructionOperandKind) -> Self {
        match kind {
            AssignedInstructionOperandKind::DataAddress { data } => Self::DataAddress { data },
            AssignedInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimeStringPointer {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Self::RuntimeStringLength {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::ImmediateInteger(value) => {
                Self::ImmediateInteger(value)
            }
            AssignedInstructionOperandKind::ByteLength(value) => Self::ByteLength(value),
        }
    }
}

pub type InstructionOperand = AssignedInstructionOperand;
pub type InstructionOperandKind = AssignedInstructionOperandKind;

impl omega_target_operations::InstructionOperandLike for AssignedInstructionOperand {
    fn data_address(&self) -> Option<omega_target_operations::TargetDataObjectHandle> {
        match self.kind {
            AssignedInstructionOperandKind::DataAddress { data } => Some(data),
            _ => None,
        }
    }

    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn immediate_integer(&self) -> Option<i64> {
        match self.kind {
            AssignedInstructionOperandKind::ImmediateInteger(value) => Some(value),
            _ => None,
        }
    }

    fn byte_length(&self) -> Option<usize> {
        match self.kind {
            AssignedInstructionOperandKind::ByteLength(value) => Some(value),
            _ => None,
        }
    }
}

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
