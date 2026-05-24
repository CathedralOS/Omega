use omega_abstract_operations::{
    FunctionInstructionPlan, InstructionOperand, RuntimeValueOperand, SelectedInstruction,
};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<RuntimeValueOperand>,
}

impl Default for InstructionPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0)
    }
}

impl InstructionPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
        }
    }
}
