pub use omega_typed_trees::{
    data, expression, identity, invariant, machine, name, platform, signature, state, types,
};

use omega_core::arena::Arena;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::sync::Arc;

pub mod statement {
    pub use omega_typed_trees::statement::*;

    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransitionGuard {
        Always,
        When(crate::expression::Expression),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TransitionTarget {
        Named {
            path: crate::expression::NamePath,
            head_symbol: SymbolHandle,
            symbol: SymbolHandle,
            arguments: HandleSpan<crate::expression::Expression>,
        },
        Value(crate::expression::Expression),
        SelfTarget,
        Terminal,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowWritableRootFact {
    pub symbol: SymbolHandle,
    pub name: name::ProgramName,
    pub kind: BorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowFact {
    pub machine_symbol: SymbolHandle,
    pub machine_name: name::ProgramName,
    pub state_symbol: SymbolHandle,
    pub state_name: name::ProgramName,
    pub writable_roots: HandleSpan<BorrowWritableRootFact>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<BorrowCallFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BorrowAccessKind {
    #[default]
    Read,
    Mutable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowArgumentAccessFact {
    pub root_name: name::ProgramName,
    pub kind: BorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowCallFact {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub receiver: Option<expression::NamePath>,
    pub target: name::ProgramName,
    pub accesses: HandleSpan<BorrowArgumentAccessFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BorrowFacts {
    pub writable_roots: Arena<BorrowWritableRootFact>,
    pub argument_accesses: Arena<BorrowArgumentAccessFact>,
    pub calls: Arena<BorrowCallFact>,
    pub states: Arena<StateBorrowFact>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligationFact {
    pub kind: ProofFactKind,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner: Arc<str>,
}

impl Default for ProofObligationFact {
    fn default() -> Self {
        Self {
            kind: ProofFactKind::default(),
            machine_symbol: SymbolHandle::invalid(),
            state_symbol: SymbolHandle::invalid(),
            owner: Arc::from(""),
        }
    }
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
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl std::ops::Deref for Program {
    type Target = omega_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<omega_typed_trees::TypedTrees> for Program {
    fn as_ref(&self) -> &omega_typed_trees::TypedTrees {
        &self.typed
    }
}
