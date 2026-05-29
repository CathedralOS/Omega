mod operand;
mod plan;
mod selected;
mod value;

pub use omega_abstract_operations::{AbstractDataObjectHandle, RuntimeStorageRegion};
pub use omega_calling_conventions::{HostBinding, HostBindingMechanism, HostOperationKey};
pub use operand::{
    InstructionOperand, InstructionOperandKind, InstructionOperandLike, TargetInstructionOperand,
    TargetInstructionOperandKind,
};
pub use plan::{InstructionPlan, TargetOperationPlan};
pub use selected::{
    FunctionInstructionPlan, SelectedInstruction, SelectedInstructionKind, TargetOperation,
    TargetOperationFunction, TargetOperationKind,
};
pub use value::{
    RuntimeValueOperand, RuntimeValueOperandHandle, RuntimeValueOperandSource, TargetValueOperand,
    TargetValueOperandHandle,
};
pub type TargetHostBinding = HostBinding;
pub type TargetBoundarySummary = omega_abstract_operations::AbstractBoundarySummary;
pub type TargetOwnershipSummary = omega_abstract_operations::AbstractOwnershipSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTextReadSource {
    HostOperation { operation_key: HostOperationKey },
}
