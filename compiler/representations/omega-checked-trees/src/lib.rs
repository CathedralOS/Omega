pub use omega_typed_trees::{
    data, expression, identity, invariant, machine, name, platform, signature, state, statement,
    tables, types,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub proof_obligation_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub typed: omega_typed_trees::Program,
    pub facts: CheckFacts,
}

impl std::ops::Deref for Program {
    type Target = omega_typed_trees::Program;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<omega_typed_trees::Program> for Program {
    fn as_ref(&self) -> &omega_typed_trees::Program {
        &self.typed
    }
}
