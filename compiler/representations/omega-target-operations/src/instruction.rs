mod plan;

pub use omega_abstract_operations::{
    FunctionInstructionPlan, HostOperationKey, InstructionOperand, InstructionOperandKind,
    RuntimeStorageRegion, RuntimeTextReadSource, RuntimeValueOperand, RuntimeValueOperandHandle,
    SelectedInstruction, SelectedInstructionKind,
};
pub use plan::InstructionPlan;
