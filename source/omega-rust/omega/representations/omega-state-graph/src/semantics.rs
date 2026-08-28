use crate::{
    ProofObligationFact, StateGraphBorrowRoots, StateGraphBoundaryRoots, StateGraphContractRoots,
    StateGraphOwnershipRoots, StateGraphValueRoots,
};
use psi_arena::Arena;
use psi_language_semantics::{ServiceReachRowTable, ServiceReachTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphServiceReachRoots {
    pub services: ServiceReachTable,
    pub rows: ServiceReachRowTable,
}

impl StateGraphServiceReachRoots {
    pub fn with_roots(services: ServiceReachTable, rows: ServiceReachRowTable) -> Self {
        Self { services, rows }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphFactRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub dynamic_conformances: psi_checked_trees::DynamicConformanceBindingFacts,
}

impl StateGraphFactRoots {
    pub fn with_roots(
        proof_obligations: Arena<ProofObligationFact>,
        dynamic_conformances: psi_checked_trees::DynamicConformanceBindingFacts,
    ) -> Self {
        Self {
            proof_obligations,
            dynamic_conformances,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphSemanticRoots {
    pub service_reach: StateGraphServiceReachRoots,
    pub facts: StateGraphFactRoots,
    pub contracts: StateGraphContractRoots,
    pub values: StateGraphValueRoots,
    pub boundaries: StateGraphBoundaryRoots,
    pub borrow: StateGraphBorrowRoots,
    pub ownership: StateGraphOwnershipRoots,
}

impl StateGraphSemanticRoots {
    pub fn with_roots(
        service_reach: StateGraphServiceReachRoots,
        facts: StateGraphFactRoots,
        contracts: StateGraphContractRoots,
        values: StateGraphValueRoots,
        boundaries: StateGraphBoundaryRoots,
        borrow: StateGraphBorrowRoots,
        ownership: StateGraphOwnershipRoots,
    ) -> Self {
        Self {
            service_reach,
            facts,
            contracts,
            values,
            boundaries,
            borrow,
            ownership,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        StateGraphBorrowRoots, StateGraphBoundaryRoots, StateGraphContractRoots,
        StateGraphFactRoots, StateGraphOwnershipRoots, StateGraphSemanticRoots,
        StateGraphServiceReachRoots, StateGraphValueRoots,
    };

    #[test]
    fn semantic_constructor_keeps_noun_roots_explicit() {
        let facts = StateGraphFactRoots::default();
        let service_reach = StateGraphServiceReachRoots::default();
        let contracts = StateGraphContractRoots::default();
        let values = StateGraphValueRoots::default();
        let boundaries = StateGraphBoundaryRoots::default();
        let borrow = StateGraphBorrowRoots::default();
        let ownership = StateGraphOwnershipRoots::default();

        let semantics = StateGraphSemanticRoots::with_roots(
            service_reach.clone(),
            facts.clone(),
            contracts.clone(),
            values.clone(),
            boundaries.clone(),
            borrow.clone(),
            ownership.clone(),
        );

        assert_eq!(semantics.service_reach, service_reach);
        assert_eq!(semantics.facts, facts);
        assert_eq!(semantics.contracts, contracts);
        assert_eq!(semantics.values, values);
        assert_eq!(semantics.boundaries, boundaries);
        assert_eq!(semantics.borrow, borrow);
        assert_eq!(semantics.ownership, ownership);
    }
}
