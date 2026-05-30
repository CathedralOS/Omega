use omega_core::arena::Arena;

use crate::{
    ControlFlowBorrowRoots, ControlFlowContractRoots, ControlFlowOwnershipRoots, InvariantFact,
    ProofObligationFact, StateBoundaryEdge, StateValueFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowFactRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowSemanticRoots {
    pub facts: ControlFlowFactRoots,
    pub contracts: ControlFlowContractRoots,
    pub values: Arena<StateValueFact>,
    pub boundary_edges: Arena<StateBoundaryEdge>,
    pub borrow: ControlFlowBorrowRoots,
    pub ownership: ControlFlowOwnershipRoots,
}
