mod capacity;
mod conversions;
mod lookups;
mod operand_handles;

use crate::{
    AssignedInstructionOperand, AssignedOperation, AssignedSemanticSummary,
    AssignedTargetOperationFunction, AssignedValueOperand,
};
use omega_target::NativeTarget;
use omega_target_operations::TargetHostBinding;
use psi_arena::Arena;

pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationCode {
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub code: AssignedTargetOperationCode,
    pub semantics: AssignedSemanticSummary,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0, 0)
    }
}

impl AssignedTargetOperationPlan {
    pub fn with_roots(
        target: NativeTarget,
        code: AssignedTargetOperationCode,
        semantics: AssignedSemanticSummary,
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
    use super::{AssignedTargetOperationCode, AssignedTargetOperationPlan};
    use crate::AssignedSemanticSummary;
    use omega_target::NativeTarget;
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let target = NativeTarget::host();
        let code = AssignedTargetOperationCode {
            functions: Arena::with_capacity(1),
            instructions: Arena::with_capacity(2),
            operands: Arena::with_capacity(3),
            runtime_value_operands: Arena::with_capacity(4),
            host_bindings: Arena::with_capacity(5),
        };
        let semantics = AssignedSemanticSummary::with_capacity(6, 7, 8, 9, 12);

        let plan = AssignedTargetOperationPlan::with_roots(target, code.clone(), semantics.clone());

        assert_eq!(plan.target, target);
        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
    }
}
