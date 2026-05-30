use crate::{
    ControlFlowBorrowRoots, ControlFlowBoundaryRoots, ControlFlowContractRoots,
    ControlFlowOwnershipRoots, ControlFlowValueRoots, InvariantFact, ProofObligationFact,
};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowFactRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
}

impl ControlFlowFactRoots {
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
pub struct ControlFlowSemanticRoots {
    pub facts: ControlFlowFactRoots,
    pub contracts: ControlFlowContractRoots,
    pub values: ControlFlowValueRoots,
    pub boundaries: ControlFlowBoundaryRoots,
    pub borrow: ControlFlowBorrowRoots,
    pub ownership: ControlFlowOwnershipRoots,
}

impl ControlFlowSemanticRoots {
    pub fn with_roots(
        facts: ControlFlowFactRoots,
        contracts: ControlFlowContractRoots,
        values: ControlFlowValueRoots,
        boundaries: ControlFlowBoundaryRoots,
        borrow: ControlFlowBorrowRoots,
        ownership: ControlFlowOwnershipRoots,
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
        ControlFlowBorrowRoots, ControlFlowBoundaryRoots, ControlFlowContractRoots,
        ControlFlowFactRoots, ControlFlowOwnershipRoots, ControlFlowSemanticRoots,
        ControlFlowValueRoots,
    };

    #[test]
    fn semantic_constructor_keeps_noun_roots_explicit() {
        let facts = ControlFlowFactRoots::default();
        let contracts = ControlFlowContractRoots::default();
        let values = ControlFlowValueRoots::default();
        let boundaries = ControlFlowBoundaryRoots::default();
        let borrow = ControlFlowBorrowRoots::default();
        let ownership = ControlFlowOwnershipRoots::default();

        let semantics = ControlFlowSemanticRoots::with_roots(
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
