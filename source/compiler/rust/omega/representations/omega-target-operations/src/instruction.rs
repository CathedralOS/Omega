mod abstract_conversions;
mod function;
mod operand;
mod operation;
mod operation_kind;
mod plan;
mod semantics;
mod value;

pub use function::{FunctionInstructionPlan, TargetOperationFunction};
pub use omega_abstract_operations::{
    AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, AbstractDataObjectHandle,
    RuntimeStorageRegion, RuntimeTextReadTarget,
};
pub use omega_calling_conventions::{HostBinding, HostBindingMechanism, HostOperationKey};
pub use operand::{
    InstructionOperand, InstructionOperandKind, InstructionOperandLike, TargetInstructionOperand,
    TargetInstructionOperandKind,
};
pub use operation::{SelectedInstruction, TargetOperation};
pub use operation_kind::{
    SelectedInstructionKind, TargetHostFormalOperandBinding, TargetHostOperationProvenance,
    TargetOperationDomain, TargetOperationKind,
};
pub use plan::{InstructionPlan, TargetOperationCode, TargetOperationPlan};
pub use semantics::{
    TargetBoundarySummary, TargetOwnershipSummary, TargetSemanticSummary, TargetValueSummary,
};
pub use value::{
    RuntimeValueOperand, RuntimeValueOperandHandle, RuntimeValueOperandSource, TargetValueOperand,
    TargetValueOperandHandle,
};
pub type TargetHostBinding = HostBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTextReadSource {
    HostOperation { operation_key: HostOperationKey },
}
