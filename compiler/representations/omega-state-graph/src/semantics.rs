use omega_core::arena::Arena;

use crate::{
    InvariantFact, ProofObligationFact, StateBorrowActivation, StateBorrowArgumentAccess,
    StateBorrowCall, StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot,
    StateBoundaryEdge, StateContractCall, StateContractExit, StateContractFactRef, StateDropEvent,
    StateMoveEvent, StateValueFact,
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
    pub borrow_writable_roots: Arena<StateBorrowWritableRoot>,
    pub borrow_access_segments: Arena<omega_facts::PlaceSegment>,
    pub borrow_argument_accesses: Arena<StateBorrowArgumentAccess>,
    pub borrow_calls: Arena<StateBorrowCall>,
    pub borrow_loans: Arena<StateBorrowLoan>,
    pub borrow_activations: Arena<StateBorrowActivation>,
    pub borrow_weakenings: Arena<StateBorrowWeakening>,
    pub ownership_segments: Arena<omega_facts::PlaceSegment>,
    pub move_events: Arena<StateMoveEvent>,
    pub drop_events: Arena<StateDropEvent>,
}
