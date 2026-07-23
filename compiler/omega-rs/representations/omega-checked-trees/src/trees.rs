use crate::{
    BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts, EffectRowFacts,
    FlowFacts, InvariantFacts, MachineContractPlans, ProofFacts, QualificationFacts,
    TerminationFacts,
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
    /// TPR3 slice 4 (decision 23): the checker-established termination
    /// summaries + completed witness elaborations.
    pub termination: TerminationFacts,
    /// STR4 slice 2 (decision 22): kinded effect rows -- published ceiling
    /// vs inferred direct/transitive summaries.
    pub effect_rows: EffectRowFacts,
    /// STR4 checked plans, slice 2 (decision 19): the semantic-domain
    /// commitments each machine's body makes (arithmetic-policy casts v1).
    pub qualifications: QualificationFacts,
    /// STR4 checked plans (machine_taxonomy.md): the normalized machine
    /// semantic contracts -- published halves + deterministic fingerprint.
    pub contract_plans: MachineContractPlans,
    /// CRY1: checker-derived four-axis carry policy per transparent data
    /// declaration. Later live-set and runtime-admission passes consume this
    /// plan rather than re-deriving policy from syntax.
    pub carry: CarryFacts,
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
        termination: TerminationFacts,
        effect_rows: EffectRowFacts,
        qualifications: QualificationFacts,
        contract_plans: MachineContractPlans,
        carry: CarryFacts,
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
            termination,
            effect_rows,
            qualifications,
            contract_plans,
            carry,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: omega_typed_trees::TypedTrees,
    pub facts: CheckFacts,
    /// Exact validated provider plans selected for this concrete checked
    /// program. Kept separate from authored candidates so backend/provider
    /// consumers cannot accidentally combine rows from different plans.
    selected_provider_plans: crate::SelectedProviderPlanFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: omega_typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        Self {
            typed,
            facts,
            selected_provider_plans: crate::SelectedProviderPlanFacts::default(),
        }
    }

    pub const fn selected_provider_plans(&self) -> &crate::SelectedProviderPlanFacts {
        &self.selected_provider_plans
    }

    /// Install a normalized selection produced from the compiler's validated
    /// candidate set. The wrapper's private storage prevents later consumers
    /// from mutating retained plan rows after checked lowering.
    pub fn retain_selected_provider_plans(&mut self, plans: crate::SelectedProviderPlanFacts) {
        self.selected_provider_plans = plans;
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
