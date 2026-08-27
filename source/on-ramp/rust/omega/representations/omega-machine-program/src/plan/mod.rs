mod capacity;
mod code;

pub use code::{MachineProgram, MachineProgramCode};

impl Default for MachineProgram {
    fn default() -> Self {
        Self::with_capacity(omega_target::NativeTarget::host(), 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use crate::{MachineProgram, MachineProgramCode, MachineSemanticSummary};
    use omega_target::NativeTarget;
    use psi_arena::Arena;

    #[test]
    fn plan_constructor_keeps_code_and_semantic_roots_explicit() {
        let target = NativeTarget::host();
        let code = MachineProgramCode {
            functions: Arena::with_capacity(1),
            instructions: Arena::with_capacity(2),
        };
        let semantics = MachineSemanticSummary::with_capacity(3, 4, 5, 6, 9);

        let program = MachineProgram::with_roots(target, code.clone(), semantics.clone());

        assert_eq!(program.target, target);
        assert_eq!(program.code, code);
        assert_eq!(program.semantics, semantics);
    }
}
