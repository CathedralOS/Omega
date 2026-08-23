use crate::{AbstractOperationCode, AbstractOperationPlan, AbstractSemanticSummary};
use psi_arena::Arena;

impl AbstractOperationPlan {
    pub fn with_capacity(
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
    ) -> Self {
        Self::with_roots(
            AbstractOperationCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
                operands: Arena::with_capacity(operand_capacity),
                runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            },
            AbstractSemanticSummary::with_capacity(0, 0, 0, 0, 0),
            Vec::new(),
        )
    }
}
