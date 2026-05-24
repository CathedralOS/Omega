pub mod data;
pub mod instruction;

pub use data::{TargetDataObject, TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan};
pub use omega_abstract_operations::{StateGuardLowering, StateGuardOperator};
pub use instruction::{
    FunctionInstructionPlan, HostOperationKey, InstructionOperand, InstructionOperandKind,
    InstructionPlan, RuntimeStorageRegion, RuntimeTextReadSource, RuntimeValueOperand,
    RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind,
};

impl From<InstructionPlan> for omega_abstract_operations::AbstractOperationPlan {
    fn from(plan: InstructionPlan) -> Self {
        Self {
            functions: plan.functions,
            instructions: plan.instructions,
            operands: plan.operands,
            runtime_value_operands: plan.runtime_value_operands,
        }
    }
}
