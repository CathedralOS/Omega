use omega_core::arena::Arena;

use crate::{
    InvariantFact, ProofObligationFact, StateBoundaryEdge, StateGraphBorrowRoots,
    StateGraphContractRoots, StateGraphOwnershipRoots, StateValueFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphSemanticRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
    pub contracts: StateGraphContractRoots,
    pub values: Arena<StateValueFact>,
    pub boundary_edges: Arena<StateBoundaryEdge>,
    pub borrow: StateGraphBorrowRoots,
    pub ownership: StateGraphOwnershipRoots,
}
