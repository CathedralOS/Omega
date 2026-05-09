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
    DataAddress { symbol: String },
    RuntimeMachineStringPointer { byte_offset: usize },
    RuntimeMachineStringLength { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
