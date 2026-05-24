mod operand;
mod plan;
mod selected;

pub use omega_abstract_operations::{AbstractDataObjectHandle, RuntimeStorageRegion};
pub use omega_calling_conventions::{HostBinding, HostBindingMechanism, HostOperationKey};
pub use operand::{InstructionOperand, InstructionOperandKind, TargetInstructionOperand, TargetInstructionOperandKind};
pub use plan::{InstructionPlan, TargetOperationPlan};
pub use selected::{
    FunctionInstructionPlan, SelectedInstruction, SelectedInstructionKind, TargetOperation,
    TargetOperationFunction, TargetOperationKind,
};

pub type TargetValueOperand = omega_abstract_operations::AbstractValueOperand;
pub type RuntimeValueOperand = TargetValueOperand;
pub type TargetValueOperandHandle = omega_abstract_operations::AbstractValueOperandHandle;
pub type RuntimeValueOperandHandle = TargetValueOperandHandle;
pub type TargetHostBinding = HostBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTextReadSource {
    HostOperation {
        operation_key: HostOperationKey,
    },
}
