use super::SelectedInstruction;
use crate::instructions::model::InstructionOperand;
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
}

impl Default for InstructionPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            operands: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInstructionPlan {
    pub symbol: String,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub instructions: HandleSpan<SelectedInstruction>,
}

impl Default for FunctionInstructionPlan {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
            instructions: HandleSpan::empty(),
        }
    }
}
