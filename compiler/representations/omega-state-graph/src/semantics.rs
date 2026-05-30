use crate::{
    InvariantFact, ProofObligationFact, StateGraphBorrowRoots, StateGraphBoundaryRoots,
    StateGraphContractRoots, StateGraphOwnershipRoots, StateGraphValueRoots,
};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphFactRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
}

impl StateGraphFactRoots {
    pub fn with_roots(
        proof_obligations: Arena<ProofObligationFact>,
        invariants: Arena<InvariantFact>,
    ) -> Self {
        Self {
            proof_obligations,
            invariants,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphSemanticRoots {
    pub facts: StateGraphFactRoots,
    pub contracts: StateGraphContractRoots,
    pub values: StateGraphValueRoots,
    pub boundaries: StateGraphBoundaryRoots,
    pub borrow: StateGraphBorrowRoots,
    pub ownership: StateGraphOwnershipRoots,
}

impl StateGraphSemanticRoots {
    pub fn with_roots(
        facts: StateGraphFactRoots,
        contracts: StateGraphContractRoots,
        values: StateGraphValueRoots,
        boundaries: StateGraphBoundaryRoots,
        borrow: StateGraphBorrowRoots,
        ownership: StateGraphOwnershipRoots,
    ) -> Self {
        Self {
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
        StateGraphValueRoots,
    };

    #[test]
    fn semantic_constructor_keeps_noun_roots_explicit() {
        let facts = StateGraphFactRoots::default();
        let contracts = StateGraphContractRoots::default();
        let values = StateGraphValueRoots::default();
        let boundaries = StateGraphBoundaryRoots::default();
        let borrow = StateGraphBorrowRoots::default();
        let ownership = StateGraphOwnershipRoots::default();

        let semantics = StateGraphSemanticRoots::with_roots(
            facts.clone(),
            contracts.clone(),
            values.clone(),
            boundaries.clone(),
            borrow.clone(),
            ownership.clone(),
        );

        assert_eq!(semantics.facts, facts);
        assert_eq!(semantics.contracts, contracts);
        assert_eq!(semantics.values, values);
        assert_eq!(semantics.boundaries, boundaries);
        assert_eq!(semantics.borrow, borrow);
        assert_eq!(semantics.ownership, ownership);
    }
}
