mod capacity;
mod lookups;

use omega_core::arena::Arena;
use omega_typed_trees::expression::ExpressionTable;

use crate::{
    ContainedGraph, InvariantFact, MachineGraph, MachineOwnedDataGraph, Operation,
    ProofObligationFact, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot, StateBoundaryEdge,
    StateContractCall, StateContractExit, StateContractFactRef, StateDropEvent, StateMoveEvent,
    StateNode, StateParameterNode, StateValueFact, TransitionEdge,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraph {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineGraph>,
    pub contained_machines: Arena<ContainedGraph>,
    pub machine_owned_data: Arena<MachineOwnedDataGraph>,
    pub states: Arena<StateNode>,
    pub state_parameters: Arena<StateParameterNode>,
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
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionEdge>,
}
