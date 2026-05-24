mod operand;
mod selected;

pub use omega_calling_conventions::HostOperationKey;
pub use operand::{InstructionOperand, InstructionOperandKind};
pub use selected::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, AbstractValueOperand,
    AbstractValueOperandHandle, FunctionInstructionPlan, RuntimeStorageRegion, RuntimeValueOperand,
    RuntimeValueOperandHandle, SelectedInstruction,
    SelectedInstructionKind,
};
