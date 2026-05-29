pub use omega_typed_trees::{
    data, expression, identity, invariant, machine, name, platform, signature, state,
    trait_definition, types,
};

mod borrow;
mod facts;
mod flow;
mod proof;

pub use borrow::*;
pub use facts::*;
pub use flow::*;
pub use proof::*;

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
pub struct CheckFacts {
    pub semantic: omega_facts::FactPlan,
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub invariants: InvariantFacts,
    pub domains: DomainFacts,
    pub effects: omega_effects::EffectPlan,
    pub flow: FlowFacts,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl std::ops::Deref for CheckedTrees {
    type Target = omega_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

impl AsRef<omega_typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &omega_typed_trees::TypedTrees {
        &self.typed
    }
}
