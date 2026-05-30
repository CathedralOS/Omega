use crate::{
    InvariantFact, ProofObligationFact, StateBoundaryEdge, StateGraphBorrowRoots,
    StateGraphContractRoots, StateGraphOwnershipRoots, StateGraphValueRoots,
};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphFactRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphSemanticRoots {
    pub facts: StateGraphFactRoots,
    pub contracts: StateGraphContractRoots,
    pub values: StateGraphValueRoots,
    pub boundary_edges: Arena<StateBoundaryEdge>,
    pub borrow: StateGraphBorrowRoots,
    pub ownership: StateGraphOwnershipRoots,
}
