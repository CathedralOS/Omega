mod operand;
mod selected;

pub use omega_calling_conventions::HostOperationKey;
pub use operand::{InstructionOperand, InstructionOperandKind};
pub use selected::{
    FunctionInstructionPlan, RuntimeStorageRegion, RuntimeTextReadSource, RuntimeValueOperand,
    RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind,
};
