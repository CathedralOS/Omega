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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowSemanticRoots {
    pub facts: ControlFlowFactRoots,
    pub contracts: ControlFlowContractRoots,
    pub values: ControlFlowValueRoots,
    pub boundaries: ControlFlowBoundaryRoots,
    pub borrow: ControlFlowBorrowRoots,
    pub ownership: ControlFlowOwnershipRoots,
}
