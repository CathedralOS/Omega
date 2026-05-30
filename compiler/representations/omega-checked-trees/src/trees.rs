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

impl CheckFacts {
    pub fn with_roots(
        semantic: omega_facts::FactPlan,
        borrow: BorrowFacts,
        proof: ProofFacts,
        values: CheckedValueFacts,
        invariants: InvariantFacts,
        domains: DomainFacts,
        effects: omega_effects::EffectPlan,
        flow: FlowFacts,
    ) -> Self {
        Self {
            semantic,
            borrow,
            proof,
            values,
            invariants,
            domains,
            effects,
            flow,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: omega_typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        Self { typed, facts }
    }
}

impl std::ops::Deref for CheckedTrees {
    type Target = omega_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

#[cfg(test)]
mod tests {
    use crate::{CheckFacts, CheckedTrees};

    #[test]
    fn checked_tree_constructor_keeps_typed_tree_and_fact_roots_explicit() {
        let typed = omega_typed_trees::TypedTrees::default();
        let facts = CheckFacts::default();

        let checked = CheckedTrees::with_roots(typed.clone(), facts.clone());

        assert_eq!(checked.typed, typed);
        assert_eq!(checked.facts, facts);
    }
}

impl AsRef<omega_typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &omega_typed_trees::TypedTrees {
        &self.typed
    }
}
