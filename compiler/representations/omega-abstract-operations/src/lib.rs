pub mod data;
pub mod instruction;

pub use data::{TargetDataObject, TargetDataObjectHandle, TargetDataObjectKind, TargetDataPlan};
pub use guard::{StateGuardLowering, StateGuardOperator};
pub use instruction::{
    AbstractFunctionPlan, AbstractOperation, AbstractOperationKind, AbstractValueOperand,
    AbstractValueOperandHandle, FunctionInstructionPlan, HostOperationKey, InstructionOperand,
    InstructionOperandKind, RuntimeStorageRegion, RuntimeTextReadSource, RuntimeValueOperand,
    RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind,
};

use omega_core::arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub functions: Arena<AbstractFunctionPlan>,
    pub instructions: Arena<AbstractOperation>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<AbstractValueOperand>,
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

mod guard {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum StateGuardOperator {
        #[default]
        None,
        Equal,
        NotEqual,
        Greater,
        GreaterOrEqual,
        Less,
        LessOrEqual,
        Add,
        Subtract,
        Multiply,
        Modulo,
        Max,
        Min,
        And,
        Or,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum StateGuardLowering {
        NoOp,
        CompareStaticValue,
        CompareRuntimeValue,
        #[default]
        NeedsRuntimeExpression,
    }
}
