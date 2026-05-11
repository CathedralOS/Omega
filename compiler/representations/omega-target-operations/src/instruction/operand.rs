#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionOperand {
    pub kind: InstructionOperandKind,
}

impl Default for InstructionOperand {
    fn default() -> Self {
        Self {
            kind: InstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionOperandKind {
    DataAddress { data: TargetDataObjectHandle },
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
use crate::RuntimeStorageRegion;
use crate::TargetDataObjectHandle;
