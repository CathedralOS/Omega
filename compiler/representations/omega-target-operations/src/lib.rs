pub mod data;
pub mod instruction;

pub use data::{TargetDataObject, TargetDataObjectHandle, TargetDataPlan};
pub use guard::{StateGuardLowering, StateGuardOperator};
pub use instruction::{
    FunctionInstructionPlan, HostOperationKey, InstructionOperand, InstructionOperandKind,
    InstructionPlan, RuntimeStorageRegion, RuntimeTextReadSource, SelectedInstruction,
    SelectedInstructionKind,
};

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
