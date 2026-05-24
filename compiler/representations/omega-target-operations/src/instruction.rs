mod plan;

pub use omega_abstract_operations::{
    HostOperationKey, InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
    RuntimeTextReadSource,
};
pub use omega_calling_conventions::{HostBinding, HostBindingMechanism};
pub use plan::{InstructionPlan, TargetOperationPlan};

pub type TargetOperationFunction = omega_abstract_operations::AbstractFunctionPlan;
pub type FunctionInstructionPlan = TargetOperationFunction;
pub type TargetOperation = omega_abstract_operations::AbstractOperation;
pub type SelectedInstruction = TargetOperation;
pub type TargetOperationKind = omega_abstract_operations::AbstractOperationKind;
pub type SelectedInstructionKind = TargetOperationKind;
pub type TargetValueOperand = omega_abstract_operations::AbstractValueOperand;
pub type RuntimeValueOperand = TargetValueOperand;
pub type TargetValueOperandHandle = omega_abstract_operations::AbstractValueOperandHandle;
pub type RuntimeValueOperandHandle = TargetValueOperandHandle;
pub type TargetHostBinding = HostBinding;
