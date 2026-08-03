mod capacity;
mod lookups;

use super::{InstructionOperand, TargetHostBinding, TargetSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationCode {
    pub functions: Arena<super::TargetOperationFunction>,
    pub instructions: Arena<super::TargetOperation>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<super::TargetValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub target: NativeTarget,
    pub code: TargetOperationCode,
    pub semantics: TargetSemanticSummary,
}

pub type InstructionPlan = TargetOperationPlan;

impl Default for TargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0)
    }
}

impl TargetOperationPlan {
    pub fn with_roots(
        target: NativeTarget,
        code: TargetOperationCode,
        semantics: TargetSemanticSummary,
    ) -> Self {
        Self {
            target,
            code,
            semantics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TargetOperationCode, TargetOperationPlan};
    use crate::TargetSemanticSummary;
    use omega_target::NativeTarget;
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let target = NativeTarget::host();
        let code = TargetOperationCode {
            functions: Arena::with_capacity(1),
            instructions: Arena::with_capacity(2),
            operands: Arena::with_capacity(3),
            runtime_value_operands: Arena::with_capacity(4),
            host_bindings: Arena::with_capacity(5),
        };
        let semantics = TargetSemanticSummary::with_capacity(6, 7, 8, 9, 12);

        let plan = TargetOperationPlan::with_roots(target, code.clone(), semantics.clone());

        assert_eq!(plan.target, target);
        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
    }
}
