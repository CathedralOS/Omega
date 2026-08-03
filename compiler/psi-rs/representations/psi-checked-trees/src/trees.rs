use crate::{
    BorrowFacts, CarryFacts, CheckedOperatorFacts, CheckedValueFacts, DomainFacts, FlowFacts,
    IndexCompatibilityFacts, InvariantFacts, MachineContractPlans, ProofFacts, QualificationFacts,
    ServiceReachFacts, TerminationFacts,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckFacts {
    pub semantic: psi_facts::FactPlan,
    pub borrow: BorrowFacts,
    pub proof: ProofFacts,
    pub values: CheckedValueFacts,
    pub invariants: InvariantFacts,
    pub domains: DomainFacts,
    pub operators: CheckedOperatorFacts,
    /// Grouped machine/state/call suspension and blocking inference.
    pub operational: psi_effects::OperationalPlan,
    pub capabilities: psi_effects::CapabilityFlowPlan,
    pub flow: FlowFacts,
    /// PDI3 named equality verification conditions and their exact discharge
    /// evidence. These rows never participate in semantic type identity.
    pub index_compatibility: IndexCompatibilityFacts,
    /// TPR3 slice 4 (decision 23): the checker-established termination
    /// summaries + completed witness elaborations.
    pub termination: TerminationFacts,
    /// EFX: symbol-resolved boundary-service declarations plus grouped
    /// machine/state/call reach summaries.
    pub service_reaches: ServiceReachFacts,
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
        semantic: psi_facts::FactPlan,
        borrow: BorrowFacts,
        proof: ProofFacts,
        values: CheckedValueFacts,
        invariants: InvariantFacts,
        domains: DomainFacts,
        operators: CheckedOperatorFacts,
        operational: psi_effects::OperationalPlan,
        capabilities: psi_effects::CapabilityFlowPlan,
        flow: FlowFacts,
        index_compatibility: IndexCompatibilityFacts,
        termination: TerminationFacts,
        service_reaches: ServiceReachFacts,
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
            operational,
            capabilities,
            flow,
            index_compatibility,
            termination,
            service_reaches,
            qualifications,
            contract_plans,
            carry,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedTrees {
    pub typed: psi_typed_trees::TypedTrees,
    pub facts: CheckFacts,
}

impl CheckedTrees {
    pub fn with_roots(typed: psi_typed_trees::TypedTrees, facts: CheckFacts) -> Self {
        Self { typed, facts }
    }
}

impl std::ops::Deref for CheckedTrees {
    type Target = psi_typed_trees::TypedTrees;

    fn deref(&self) -> &Self::Target {
        &self.typed
    }
}

#[cfg(test)]
mod tests {
    use crate::{CheckFacts, CheckedTrees};

    #[test]
    fn checked_tree_constructor_keeps_typed_tree_and_fact_roots_explicit() {
        let typed = psi_typed_trees::TypedTrees::default();
        let facts = CheckFacts::default();

        let checked = CheckedTrees::with_roots(typed.clone(), facts.clone());

        assert_eq!(checked.typed, typed);
        assert_eq!(checked.facts, facts);
    }
}

impl AsRef<psi_typed_trees::TypedTrees> for CheckedTrees {
    fn as_ref(&self) -> &psi_typed_trees::TypedTrees {
        &self.typed
    }
}
