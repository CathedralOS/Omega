pub mod data;
pub mod instruction;

pub use data::{TargetDataObject, TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan};
pub use omega_abstract_operations::{StateGuardLowering, StateGuardOperator};
pub use instruction::{
    FunctionInstructionPlan, HostOperationKey, InstructionOperand, InstructionOperandKind,
    InstructionPlan, RuntimeStorageRegion, RuntimeTextReadSource, RuntimeValueOperand,
    RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind,
};
