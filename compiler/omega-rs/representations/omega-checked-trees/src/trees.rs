use crate::{
    BorrowFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts, FlowFacts, InvariantFacts,
    ProofFacts,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub semantic: omega_facts::FactPlan,
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub values: CheckedValueFacts,
    pub invariants: InvariantFacts,
    pub domains: DomainFacts,
    pub operators: CheckedOperatorFacts,
    pub effects: omega_effects::EffectPlan,
    pub capabilities: omega_effects::CapabilityFlowPlan,
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
        operators: CheckedOperatorFacts,
        effects: omega_effects::EffectPlan,
        capabilities: omega_effects::CapabilityFlowPlan,
        flow: FlowFacts,
    ) -> Self {
        Self {
            semantic,
            borrow,
            proof,
            values,
            invariants,
            domains,
            operators,
            effects,
            capabilities,
            flow,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
    /// Exact completion summary derived for local checked consumers. This is
    /// deliberately separate from `Machine::termination_guarantee`, which is
    /// the authored or inherited published interface.
    pub termination_summaries: Vec<MachineTerminationSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineTerminationSummary {
    pub machine: omega_core::symbols::SymbolHandle,
    pub guarantee: omega_core::termination::TerminationGuarantee,
}

impl CheckedTrees {
    pub fn with_roots(typed: omega_typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        let termination_summaries = typed
            .machines()
            .iter()
            .map(|machine| MachineTerminationSummary {
                machine: machine.symbol,
                guarantee: machine.termination_guarantee,
            })
            .collect();
        Self {
            typed,
            facts,
            termination_summaries,
        }
    }

    pub fn with_termination_summaries(
        typed: omega_typed_trees::TypedTrees,
        facts: CheckFacts,
        termination_summaries: Vec<MachineTerminationSummary>,
    ) -> Self {
        Self {
            typed,
            facts,
            termination_summaries,
        }
    }

    pub fn machine_termination_summary(
        &self,
        machine: omega_core::symbols::SymbolHandle,
    ) -> omega_core::termination::TerminationGuarantee {
        self.termination_summaries
            .iter()
            .find(|summary| summary.machine == machine)
            .map(|summary| summary.guarantee)
            .unwrap_or(omega_core::termination::TerminationGuarantee::None)
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
