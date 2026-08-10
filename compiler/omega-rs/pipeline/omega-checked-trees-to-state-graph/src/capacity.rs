mod expressions;

use omega_state_graph::StateGraph;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::ExpressionTableCapacity;
use psi_checked_trees::machine::Machine;

use crate::capacity::expressions::machine_expression_capacity;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StateGraphCapacity {
    code: StateGraphCodeCapacity,
    semantics: StateGraphSemanticCapacity,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphCodeCapacity {
    expressions: ExpressionTableCapacity,
    machines: usize,
    contained_machines: usize,
    machine_owned_data: usize,
    states: usize,
    state_parameters: usize,
    operations: usize,
    transitions: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphSemanticCapacity {
    facts: StateGraphFactCapacity,
    contracts: StateGraphContractCapacity,
    values: StateGraphValueCapacity,
    boundaries: StateGraphBoundaryCapacity,
    borrow: StateGraphBorrowCapacity,
    ownership: StateGraphOwnershipCapacity,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphFactCapacity {
    proof_obligations: usize,
    invariants: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphContractCapacity {
    contract_fact_refs: usize,
    contract_calls: usize,
    contract_exits: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphValueCapacity {
    values: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphBoundaryCapacity {
    boundary_edges: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphBorrowCapacity {
    borrow_writable_roots: usize,
    borrow_access_segments: usize,
    borrow_argument_accesses: usize,
    borrow_calls: usize,
    borrow_loans: usize,
    borrow_activations: usize,
    borrow_weakenings: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphOwnershipCapacity {
    ownership_segments: usize,
    permission_events: usize,
}

impl StateGraphCapacity {
    pub(crate) fn for_program(program: &CheckedTrees) -> Self {
        Self {
            code: StateGraphCodeCapacity::for_program(program),
            semantics: StateGraphSemanticCapacity::for_program(program),
        }
    }

    pub(crate) fn for_machine(program: &CheckedTrees, machine: &Machine) -> Self {
        Self {
            code: StateGraphCodeCapacity::for_machine(program, machine),
            semantics: StateGraphSemanticCapacity::for_machine(program, machine),
        }
    }

    pub(crate) fn into_state_graph(self) -> StateGraph {
        StateGraph::with_capacity(
            self.code.expressions,
            self.code.machines,
            self.code.contained_machines,
            self.code.machine_owned_data,
            self.code.states,
            self.code.state_parameters,
            self.semantics.facts.proof_obligations,
            self.semantics.facts.invariants,
            self.semantics.contracts.contract_fact_refs,
            self.semantics.contracts.contract_calls,
            self.semantics.contracts.contract_exits,
            self.semantics.values.values,
            self.semantics.boundaries.boundary_edges,
            self.semantics.borrow.borrow_writable_roots,
            self.semantics.borrow.borrow_access_segments,
            self.semantics.borrow.borrow_argument_accesses,
            self.semantics.borrow.borrow_calls,
            self.semantics.borrow.borrow_loans,
            self.semantics.borrow.borrow_activations,
            self.semantics.borrow.borrow_weakenings,
            self.semantics.ownership.ownership_segments,
            self.semantics.ownership.permission_events,
            self.code.operations,
            self.code.transitions,
        )
    }
}

impl StateGraphCodeCapacity {
    fn for_program(program: &CheckedTrees) -> Self {
        let mut capacity = Self {
            expressions: ExpressionTableCapacity::default(),
            machines: program.machines().len(),
            contained_machines: program.facts.carry.contained_fields.len(),
            machine_owned_data: program.machine_owned_data.len(),
            states: 0,
            state_parameters: program.state_parameters.len(),
            operations: program.statement_table.statement_count(),
            transitions: program.statement_table.transition_target_count(),
        };

        for machine in program.machines() {
            capacity.states = capacity
                .states
                .saturating_add(estimated_machine_segment_capacity(program, machine));
            capacity
                .expressions
                .saturating_add_assign(machine_expression_capacity(program, machine));
        }

        capacity
    }

    fn for_machine(program: &CheckedTrees, machine: &Machine) -> Self {
        let statement_capacity = machine_statement_count(program, machine);

        Self {
            expressions: machine_expression_capacity(program, machine),
            machines: 1,
            contained_machines: program
                .facts
                .carry
                .contained_fields_for_machine(machine.symbol)
                .len(),
            machine_owned_data: program.machine_owned_data(machine).len(),
            states: estimated_machine_segment_capacity(program, machine),
            state_parameters: machine_state_parameter_count(program, machine),
            operations: statement_capacity,
            transitions: statement_capacity,
        }
    }
}

impl StateGraphSemanticCapacity {
    fn for_program(program: &CheckedTrees) -> Self {
        Self {
            facts: StateGraphFactCapacity {
                proof_obligations: program.facts.proof.obligations.len(),
                invariants: program.facts.invariants.definitions.len(),
            },
            contracts: StateGraphContractCapacity {
                contract_fact_refs: program.facts.proof.contract_fact_refs.len(),
                contract_calls: program.facts.proof.contract_calls.len(),
                contract_exits: program.facts.proof.contract_exits.len(),
            },
            values: StateGraphValueCapacity {
                values: program.facts.values.values.len(),
            },
            boundaries: StateGraphBoundaryCapacity {
                boundary_edges: program.facts.flow.boundaries.edges.len(),
            },
            borrow: StateGraphBorrowCapacity::for_program(program),
            ownership: StateGraphOwnershipCapacity::for_program(program),
        }
    }

    fn for_machine(program: &CheckedTrees, machine: &Machine) -> Self {
        Self {
            facts: StateGraphFactCapacity {
                proof_obligations: 0,
                invariants: 0,
            },
            contracts: StateGraphContractCapacity {
                contract_fact_refs: machine_contract_fact_ref_count(program, machine),
                contract_calls: machine_contract_call_count(program, machine),
                contract_exits: machine_contract_exit_count(program, machine),
            },
            values: StateGraphValueCapacity {
                values: machine_value_count(program, machine),
            },
            boundaries: StateGraphBoundaryCapacity {
                boundary_edges: machine_boundary_edge_count(program, machine),
            },
            borrow: StateGraphBorrowCapacity::for_program(program),
            ownership: StateGraphOwnershipCapacity::for_program(program),
        }
    }
}

impl StateGraphBorrowCapacity {
    fn for_program(program: &CheckedTrees) -> Self {
        Self {
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_access_segments: program.facts.borrow.access_segments.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            borrow_loans: program.facts.borrow.loans.len(),
            borrow_activations: program.facts.flow.borrow_lifetimes.activations.len(),
            borrow_weakenings: program.facts.flow.borrow_lifetimes.weakenings.len(),
        }
    }
}

impl StateGraphOwnershipCapacity {
    fn for_program(program: &CheckedTrees) -> Self {
        Self {
            ownership_segments: program.facts.flow.ownership.segments.len(),
            permission_events: program.facts.flow.ownership.permissions.len(),
        }
    }
}

fn machine_boundary_edge_count(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
        .map(|(_, state)| state.boundary_edges.len())
        .sum()
}

fn machine_value_count(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .facts
        .values
        .values
        .iter()
        .filter(|(_, value)| {
            matches!(
                value.origin,
                psi_checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. }
                    if machine_symbol == machine.symbol
            )
        })
        .count()
}

pub(crate) fn machine_statement_count(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .machine_states(machine)
        .iter()
        .map(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .len()
        })
        .sum()
}

fn machine_state_parameter_count(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .machine_states(machine)
        .iter()
        .map(|state| program.state_parameters(state).len())
        .sum()
}

fn machine_contract_call_count(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .facts
        .proof
        .contract_calls
        .iter()
        .filter(|(_, call)| call.caller_machine_symbol == machine.symbol)
        .count()
}

fn machine_contract_exit_count(program: &CheckedTrees, machine: &Machine) -> usize {
    program
        .facts
        .proof
        .contract_exits
        .iter()
        .filter(|(_, exit)| exit.machine_symbol == machine.symbol)
        .count()
}

fn machine_contract_fact_ref_count(program: &CheckedTrees, machine: &Machine) -> usize {
    let call_refs = program
        .facts
        .proof
        .contract_calls
        .iter()
        .filter(|(_, call)| call.caller_machine_symbol == machine.symbol)
        .map(|(_, call)| call.requires.len().saturating_add(call.ensures.len()))
        .sum::<usize>();
    let exit_refs = program
        .facts
        .proof
        .contract_exits
        .iter()
        .filter(|(_, exit)| exit.machine_symbol == machine.symbol)
        .map(|(_, exit)| exit.ensures.len())
        .sum::<usize>();

    call_refs.saturating_add(exit_refs)
}

pub(crate) fn estimated_machine_segment_capacity(
    program: &CheckedTrees,
    machine: &Machine,
) -> usize {
    program
        .machine_states(machine)
        .iter()
        .map(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .len()
                .max(1)
        })
        .sum()
}
