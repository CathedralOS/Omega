use omega_core::arena::Arena;

use crate::{
    InvariantFact, ProofObligationFact, StateBoundaryEdge, StateContractCall, StateContractExit,
    StateContractFactRef, StateGraphBorrowRoots, StateGraphOwnershipRoots, StateValueFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphSemanticRoots {
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
    pub contract_fact_refs: Arena<StateContractFactRef>,
    pub contract_calls: Arena<StateContractCall>,
    pub contract_exits: Arena<StateContractExit>,
    pub values: Arena<StateValueFact>,
    pub boundary_edges: Arena<StateBoundaryEdge>,
    pub borrow: StateGraphBorrowRoots,
    pub ownership: StateGraphOwnershipRoots,
}
