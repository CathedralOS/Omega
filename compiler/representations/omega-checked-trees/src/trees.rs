use crate::{BorrowFacts, CheckedValueFacts, DomainFacts, FlowFacts, InvariantFacts, ProofFacts};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub semantic: omega_facts::FactPlan,
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub values: CheckedValueFacts,
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
