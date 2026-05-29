use crate::{
    AbstractFunctionPlan, AbstractOperation, AbstractSemanticSummary, AbstractValueOperand,
    InstructionOperand,
};
use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub functions: Arena<AbstractFunctionPlan>,
    pub instructions: Arena<AbstractOperation>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<AbstractValueOperand>,
    pub semantics: AbstractSemanticSummary,
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
            semantics: AbstractSemanticSummary::default(),
        }
    }
}
