use omega_core::arena::Arena;
use omega_typed_trees::expression::{ExpressionTable, ExpressionTableCapacity};

use crate::{StateGraph, StateGraphSemanticRoots};

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
            semantics: StateGraphSemanticRoots {
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
            },
            operations: Arena::with_capacity(operation_capacity),
            transitions: Arena::with_capacity(transition_capacity),
        }
    }
}
