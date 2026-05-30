use omega_core::arena::Arena;

use crate::{
    ControlFlowBorrowRoots, ControlFlowOwnershipRoots, InvariantFact, ProofObligationFact,
    StateBoundaryEdge, StateContractCall, StateContractExit, StateContractFactRef, StateValueFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowSemanticRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
    pub contract_fact_refs: Arena<StateContractFactRef>,
    pub contract_calls: Arena<StateContractCall>,
    pub contract_exits: Arena<StateContractExit>,
    pub values: Arena<StateValueFact>,
    pub boundary_edges: Arena<StateBoundaryEdge>,
    pub borrow: ControlFlowBorrowRoots,
    pub ownership: ControlFlowOwnershipRoots,
}
