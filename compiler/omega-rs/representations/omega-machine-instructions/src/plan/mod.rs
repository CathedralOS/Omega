mod capacity;
mod code;

pub use code::{MachineInstructionCode, MachineInstructionPlan};

impl Default for MachineInstructionPlan {
    fn default() -> Self {
        Self::with_capacity(omega_target::NativeTarget::host(), 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        MachineInstructionCode, MachineInstructionPlan, MachineInstructionSemanticSummary,
    };
    use omega_target::NativeTarget;
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let target = NativeTarget::host();
        let code = MachineInstructionCode {
            functions: Arena::with_capacity(1),
            instructions: Arena::with_capacity(2),
        };
        let semantics = MachineInstructionSemanticSummary::with_capacity(3, 4, 5, 6, 9);

        let plan = MachineInstructionPlan::with_roots(target, code.clone(), semantics.clone());

        assert_eq!(plan.target, target);
        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
    }
}
