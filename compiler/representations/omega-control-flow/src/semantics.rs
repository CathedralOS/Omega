use omega_core::arena::Arena;

use crate::{
    ControlFlowBorrowRoots, ControlFlowContractRoots, ControlFlowOwnershipRoots,
    ControlFlowValueRoots, InvariantFact, ProofObligationFact, StateBoundaryEdge,
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
    pub values: ControlFlowValueRoots,
    pub boundary_edges: Arena<StateBoundaryEdge>,
    pub borrow: ControlFlowBorrowRoots,
    pub ownership: ControlFlowOwnershipRoots,
}
