mod capacity;
mod code;

pub use code::{EncodedMachineCode, EncodedMachinePlan};

impl Default for EncodedMachinePlan {
    fn default() -> Self {
        Self::with_capacity(omega_target::NativeTarget::host(), 0, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{EncodedMachineCode, EncodedMachinePlan, EncodedMachineSemanticSummary};
    use omega_target::NativeTarget;
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let target = NativeTarget::host();
        let code = EncodedMachineCode {
            functions: Arena::with_capacity(1),
            instructions: Arena::with_capacity(2),
            bytes: Arena::with_capacity(3),
            runtime_value_operands: Arena::with_capacity(4),
            byte_count: 4,
        };
        let semantics = EncodedMachineSemanticSummary::with_capacity(5, 6, 7, 8, 11);

        let plan = EncodedMachinePlan::with_roots(target, code.clone(), semantics.clone());

        assert_eq!(plan.target, target);
        assert_eq!(plan.code, code);
        assert_eq!(plan.semantics, semantics);
    }
}
