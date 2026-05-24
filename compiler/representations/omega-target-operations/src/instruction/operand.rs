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
    fn instruction_operand_kind(&self) -> &InstructionOperandKind;
}

impl InstructionOperandLike for TargetInstructionOperand {
    fn instruction_operand_kind(&self) -> &InstructionOperandKind {
        &self.kind
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
