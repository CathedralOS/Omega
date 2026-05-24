use crate::RuntimeStorageRegion;
use crate::TargetDataObjectHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInstructionOperand {
    pub kind: TargetInstructionOperandKind,
}

impl Default for TargetInstructionOperand {
    fn default() -> Self {
        Self {
            kind: TargetInstructionOperandKind::ImmediateInteger(0),
        }
    }
}

pub trait InstructionOperandLike {
    fn data_address(&self) -> Option<TargetDataObjectHandle>;
    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)>;
    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)>;
    fn immediate_integer(&self) -> Option<i64>;
    fn byte_length(&self) -> Option<usize>;
}

impl InstructionOperandLike for TargetInstructionOperand {
    fn data_address(&self) -> Option<TargetDataObjectHandle> {
        match self.kind {
            InstructionOperandKind::DataAddress { data } => Some(data),
            _ => None,
        }
    }

    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            InstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn immediate_integer(&self) -> Option<i64> {
        match self.kind {
            InstructionOperandKind::ImmediateInteger(value) => Some(value),
            _ => None,
        }
    }

    fn byte_length(&self) -> Option<usize> {
        match self.kind {
            InstructionOperandKind::ByteLength(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetInstructionOperandKind {
    DataAddress {
        data: TargetDataObjectHandle,
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

pub type InstructionOperand = TargetInstructionOperand;
pub type InstructionOperandKind = TargetInstructionOperandKind;
