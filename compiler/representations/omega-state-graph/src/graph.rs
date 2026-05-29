use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionTable, ExpressionTableCapacity};
use omega_typed_trees::name::Identifier;

use crate::{
    ContainedGraph, InvariantFact, MachineGraph, MachineOwnedDataGraph, Operation,
    ProofObligationFact, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot, StateBoundaryEdge,
    StateContractCall, StateContractExit, StateContractFactRef, StateDropEvent, StateKey,
    StateMoveEvent, StateNode, StateParameterNode, StateValueFact, TransitionEdge,
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

impl StateGraph {
    pub fn with_capacity(
        expression_capacity: ExpressionTableCapacity,
        machine_capacity: usize,
        contained_machine_capacity: usize,
        machine_owned_data_capacity: usize,
        state_capacity: usize,
        state_parameter_capacity: usize,
        proof_obligation_capacity: usize,
        invariant_capacity: usize,
        contract_fact_ref_capacity: usize,
        contract_call_capacity: usize,
        contract_exit_capacity: usize,
        value_capacity: usize,
        boundary_edge_capacity: usize,
        borrow_writable_root_capacity: usize,
        borrow_access_segment_capacity: usize,
        borrow_argument_access_capacity: usize,
        borrow_call_capacity: usize,
        borrow_loan_capacity: usize,
        borrow_activation_capacity: usize,
        borrow_weakening_capacity: usize,
        ownership_segment_capacity: usize,
        move_event_capacity: usize,
        drop_event_capacity: usize,
        operation_capacity: usize,
        transition_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_capacities(expression_capacity),
            machines: Arena::with_capacity(machine_capacity),
            contained_machines: Arena::with_capacity(contained_machine_capacity),
            machine_owned_data: Arena::with_capacity(machine_owned_data_capacity),
            states: Arena::with_capacity(state_capacity),
            state_parameters: Arena::with_capacity(state_parameter_capacity),
            proof_obligations: Arena::with_capacity(proof_obligation_capacity),
            invariants: Arena::with_capacity(invariant_capacity),
            contract_fact_refs: Arena::with_capacity(contract_fact_ref_capacity),
            contract_calls: Arena::with_capacity(contract_call_capacity),
            contract_exits: Arena::with_capacity(contract_exit_capacity),
            values: Arena::with_capacity(value_capacity),
            boundary_edges: Arena::with_capacity(boundary_edge_capacity),
            borrow_writable_roots: Arena::with_capacity(borrow_writable_root_capacity),
            borrow_access_segments: Arena::with_capacity(borrow_access_segment_capacity),
            borrow_argument_accesses: Arena::with_capacity(borrow_argument_access_capacity),
            borrow_calls: Arena::with_capacity(borrow_call_capacity),
            borrow_loans: Arena::with_capacity(borrow_loan_capacity),
            borrow_activations: Arena::with_capacity(borrow_activation_capacity),
            borrow_weakenings: Arena::with_capacity(borrow_weakening_capacity),
            ownership_segments: Arena::with_capacity(ownership_segment_capacity),
            move_events: Arena::with_capacity(move_event_capacity),
            drop_events: Arena::with_capacity(drop_event_capacity),
            operations: Arena::with_capacity(operation_capacity),
            transitions: Arena::with_capacity(transition_capacity),
        }
    }

    pub fn state_key_by_symbols(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<StateKey> {
        let machine = self.machine_by_symbol(machine_symbol)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key.machine == machine_symbol && state.key.state == state_symbol)
            .map(|state| state.key)
    }

    pub fn machine_by_symbol(&self, machine_symbol: SymbolHandle) -> Option<&MachineGraph> {
        self.machines
            .iter()
            .find(|(_, machine)| machine.symbol == machine_symbol)
            .map(|(_, machine)| machine)
    }

    pub fn machine_contains(&self, machine: &MachineGraph) -> &[ContainedGraph] {
        self.contained_machines.span_or_empty(machine.contains)
    }

    pub fn machine_owned_data(&self, machine: &MachineGraph) -> &[MachineOwnedDataGraph] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn state_by_key(&self, key: StateKey) -> Option<&StateNode> {
        let machine = self.machine_by_symbol(key.machine)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)
    }

    pub fn state_parameters(&self, state: &StateNode) -> &[StateParameterNode] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn state_names_by_key(&self, key: StateKey) -> Option<(&Identifier, &Identifier)> {
        let machine = self.machine_by_symbol(key.machine)?;
        let state = self
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)?;

        Some((&machine.name, &state.name))
    }

    pub fn state_names_by_key_cloned(&self, key: StateKey) -> (Identifier, Identifier) {
        self.state_names_by_key(key)
            .map(|(machine, state)| (machine.clone(), state.clone()))
            .unwrap_or_default()
    }

    pub fn state_machine_name_by_key_cloned(&self, key: StateKey) -> Identifier {
        self.state_names_by_key(key)
            .map(|(machine, _)| machine.clone())
            .unwrap_or_default()
    }

    pub fn state_name_by_key_cloned(&self, key: StateKey) -> Identifier {
        self.state_names_by_key(key)
            .map(|(_, state)| state.clone())
            .unwrap_or_default()
    }
}
