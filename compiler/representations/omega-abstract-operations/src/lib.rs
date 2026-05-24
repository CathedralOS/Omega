use omega_core::arena::Arena;

pub use omega_target_operations::{
    FunctionInstructionPlan, InstructionOperand, RuntimeValueOperand, SelectedInstruction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<RuntimeValueOperand>,
}

impl Default for AbstractOperationPlan {
    fn default() -> Self {
        Self::with_capacity(0, 0, 0, 0)
    }
}

impl AbstractOperationPlan {
    pub fn with_capacity(
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
    ) -> Self {
        Self {
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
        }
    }
}

impl From<omega_target_operations::InstructionPlan> for AbstractOperationPlan {
    fn from(plan: omega_target_operations::InstructionPlan) -> Self {
        Self {
            functions: plan.functions,
            instructions: plan.instructions,
            operands: plan.operands,
            runtime_value_operands: plan.runtime_value_operands,
        }
    }
}
