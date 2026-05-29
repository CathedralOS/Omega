mod abstract_conversions;
mod function;
mod operand;
mod operation;
mod operation_kind;
mod plan;
mod value;

pub use function::{FunctionInstructionPlan, TargetOperationFunction};
pub use omega_abstract_operations::{AbstractDataObjectHandle, RuntimeStorageRegion};
pub use omega_calling_conventions::{HostBinding, HostBindingMechanism, HostOperationKey};
pub use operand::{
    InstructionOperand, InstructionOperandKind, InstructionOperandLike, TargetInstructionOperand,
    TargetInstructionOperandKind,
};
pub use operation::{SelectedInstruction, TargetOperation};
pub use operation_kind::{SelectedInstructionKind, TargetOperationKind};
pub use plan::{InstructionPlan, TargetOperationPlan};
pub use value::{
    RuntimeValueOperand, RuntimeValueOperandHandle, RuntimeValueOperandSource, TargetValueOperand,
    TargetValueOperandHandle,
};
pub type TargetHostBinding = HostBinding;
pub type TargetBoundarySummary = omega_abstract_operations::AbstractBoundarySummary;
pub type TargetOwnershipSummary = omega_abstract_operations::AbstractOwnershipSummary;
pub type TargetValueSummary = omega_abstract_operations::AbstractValueSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTextReadSource {
    HostOperation { operation_key: HostOperationKey },
}
