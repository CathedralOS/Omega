use crate::{
    ControlFlowBorrowRoots, ControlFlowBoundaryRoots, ControlFlowContractRoots,
    ControlFlowOwnershipRoots, ControlFlowValueRoots, InvariantFact, ProofObligationFact,
};
use psi_arena::Arena;
use psi_language_semantics::{ServiceReachRowTable, ServiceReachTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowServiceReachRoots {
    pub services: ServiceReachTable,
    pub rows: ServiceReachRowTable,
}

impl ControlFlowServiceReachRoots {
    pub fn with_roots(services: ServiceReachTable, rows: ServiceReachRowTable) -> Self {
        Self { services, rows }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowFactRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
    pub dynamic_conformances: psi_checked_trees::DynamicConformanceBindingFacts,
}

impl ControlFlowFactRoots {
    pub fn with_roots(
        proof_obligations: Arena<ProofObligationFact>,
        invariants: Arena<InvariantFact>,
        dynamic_conformances: psi_checked_trees::DynamicConformanceBindingFacts,
    ) -> Self {
        Self {
            proof_obligations,
            invariants,
            dynamic_conformances,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowSemanticRoots {
    pub service_reach: ControlFlowServiceReachRoots,
    pub facts: ControlFlowFactRoots,
    pub contracts: ControlFlowContractRoots,
    pub values: ControlFlowValueRoots,
    pub boundaries: ControlFlowBoundaryRoots,
    pub borrow: ControlFlowBorrowRoots,
    pub ownership: ControlFlowOwnershipRoots,
}

impl ControlFlowSemanticRoots {
    pub fn with_roots(
        service_reach: ControlFlowServiceReachRoots,
        facts: ControlFlowFactRoots,
        contracts: ControlFlowContractRoots,
        values: ControlFlowValueRoots,
        boundaries: ControlFlowBoundaryRoots,
        borrow: ControlFlowBorrowRoots,
        ownership: ControlFlowOwnershipRoots,
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
        ControlFlowBorrowRoots, ControlFlowBoundaryRoots, ControlFlowContractRoots,
        ControlFlowFactRoots, ControlFlowOwnershipRoots, ControlFlowSemanticRoots,
        ControlFlowServiceReachRoots, ControlFlowValueRoots,
    };

    #[test]
    fn semantic_constructor_keeps_noun_roots_explicit() {
        let facts = ControlFlowFactRoots::default();
        let service_reach = ControlFlowServiceReachRoots::default();
        let contracts = ControlFlowContractRoots::default();
        let values = ControlFlowValueRoots::default();
        let boundaries = ControlFlowBoundaryRoots::default();
        let borrow = ControlFlowBorrowRoots::default();
        let ownership = ControlFlowOwnershipRoots::default();

        let semantics = ControlFlowSemanticRoots::with_roots(
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
