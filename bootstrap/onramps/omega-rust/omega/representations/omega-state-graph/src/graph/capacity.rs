use psi_arena::Arena;
use psi_typed_trees::expression::{ExpressionTable, ExpressionTableCapacity};

use crate::{
    StateGraph, StateGraphBorrowRoots, StateGraphBoundaryRoots, StateGraphCode,
    StateGraphContractRoots, StateGraphFactRoots, StateGraphOwnershipRoots,
    StateGraphSemanticRoots, StateGraphServiceReachRoots, StateGraphValueRoots,
};

impl StateGraph {
    pub fn with_capacity(
        expression_capacity: ExpressionTableCapacity,
        machine_capacity: usize,
        contained_machine_capacity: usize,
        machine_owned_data_capacity: usize,
        state_capacity: usize,
        state_parameter_capacity: usize,
        proof_obligation_capacity: usize,
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
        permission_event_capacity: usize,
        operation_capacity: usize,
        transition_capacity: usize,
    ) -> Self {
        Self::with_roots(
            StateGraphCode::with_roots(
                ExpressionTable::with_capacities(expression_capacity),
                Arena::with_capacity(machine_capacity),
                Arena::with_capacity(contained_machine_capacity),
                Arena::with_capacity(machine_owned_data_capacity),
                Arena::with_capacity(state_capacity),
                Arena::with_capacity(state_parameter_capacity),
                Arena::with_capacity(operation_capacity),
                Arena::with_capacity(transition_capacity),
            ),
            StateGraphSemanticRoots::with_roots(
                StateGraphServiceReachRoots::default(),
                StateGraphFactRoots::with_roots(
                    Arena::with_capacity(proof_obligation_capacity),
                    Default::default(),
                ),
                StateGraphContractRoots::with_roots(
                    Arena::with_capacity(contract_fact_ref_capacity),
                    Arena::with_capacity(contract_call_capacity),
                    Arena::with_capacity(contract_exit_capacity),
                ),
                StateGraphValueRoots::with_roots(Arena::with_capacity(value_capacity)),
                StateGraphBoundaryRoots::with_roots(Arena::with_capacity(boundary_edge_capacity)),
                StateGraphBorrowRoots::with_roots(
                    Arena::with_capacity(borrow_writable_root_capacity),
                    Arena::with_capacity(borrow_access_segment_capacity),
                    Arena::with_capacity(borrow_argument_access_capacity),
                    Arena::with_capacity(borrow_call_capacity),
                    Arena::with_capacity(borrow_loan_capacity),
                    Arena::with_capacity(borrow_activation_capacity),
                    Arena::with_capacity(borrow_weakening_capacity),
                ),
                StateGraphOwnershipRoots::with_roots(
                    Arena::with_capacity(ownership_segment_capacity),
                    Arena::with_capacity(permission_event_capacity),
                ),
            ),
        )
    }
}
