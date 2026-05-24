pub mod data;
pub mod instruction;

pub use data::{
    TargetDataObject, TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan,
    target_data_handle_from_abstract,
};
pub use omega_abstract_operations::{StateGuardLowering, StateGuardOperator};
pub use instruction::{
    AbstractDataObjectHandle, FunctionInstructionPlan, HostOperationKey, InstructionOperand,
    InstructionOperandKind, InstructionPlan, RuntimeStorageRegion, RuntimeTextReadSource,
    RuntimeValueOperand, RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind,
    TargetHostBinding, TargetOperation, TargetOperationFunction, TargetOperationKind,
    TargetOperationPlan, TargetValueOperand, TargetValueOperandHandle,
};
