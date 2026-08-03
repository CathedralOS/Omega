use crate::{
    AbstractFunctionPlan, AbstractOperation, AbstractSemanticSummary, AbstractValueOperand,
    InstructionOperand,
};
use psi_arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationCode {
    pub functions: Arena<AbstractFunctionPlan>,
    pub instructions: Arena<AbstractOperation>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<AbstractValueOperand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlan {
    pub code: AbstractOperationCode,
    pub semantics: AbstractSemanticSummary,
    pub permission_realization_candidates: Vec<crate::PermissionRealizationCandidate>,
}

impl AbstractOperationPlan {
    pub fn with_roots(
        code: AbstractOperationCode,
        semantics: AbstractSemanticSummary,
        permission_realization_candidates: Vec<crate::PermissionRealizationCandidate>,
    ) -> Self {
        Self {
            code,
            semantics,
            permission_realization_candidates,
        }
    }
}
