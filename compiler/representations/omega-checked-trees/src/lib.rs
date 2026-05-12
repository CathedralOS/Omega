pub use omega_typed_trees::{
    data, expression, identity, invariant, machine, name, platform, signature, state, statement,
    tables, types,
};

use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofFactKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofObligationFact {
    pub kind: ProofFactKind,
    pub owner: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProofFacts {
    pub obligations: Arena<ProofObligationFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFact {
    pub symbol: SymbolHandle,
    pub name: name::ProgramName,
    pub constraint_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFacts {
    pub definitions: Arena<InvariantFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub invariants: InvariantFacts,
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
