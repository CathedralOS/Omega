pub mod data;
pub mod instruction;

pub use data::{
    TargetDataObject, TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan,
    target_data_handle_from_abstract,
};
pub use instruction::{
    AbstractDataObjectHandle, FunctionInstructionPlan, HostOperationKey, InstructionOperand,
    InstructionOperandKind, InstructionOperandLike, InstructionPlan, RuntimeStorageRegion,
    RuntimeTextReadSource, RuntimeValueOperand, RuntimeValueOperandHandle,
    RuntimeValueOperandSource, SelectedInstruction, SelectedInstructionKind, TargetBoundarySummary,
    TargetHostBinding, TargetInstructionOperand, TargetInstructionOperandKind, TargetOperation,
    TargetOperationFunction, TargetOperationKind, TargetOperationPlan, TargetOwnershipSummary,
    TargetSemanticSummary, TargetValueOperand, TargetValueOperandHandle, TargetValueSummary,
};
pub use omega_abstract_operations::{StateGuardLowering, StateGuardOperator};
