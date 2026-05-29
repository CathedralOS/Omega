mod expressions;

use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::ExpressionTableCapacity;
use omega_checked_trees::machine::Machine;
use omega_state_graph::StateGraph;

use crate::capacity::expressions::machine_expression_capacity;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StateGraphCapacity {
    expressions: ExpressionTableCapacity,
    machines: usize,
    contained_machines: usize,
    machine_owned_data: usize,
    states: usize,
    state_parameters: usize,
    proof_obligations: usize,
    invariants: usize,
    contract_fact_refs: usize,
    contract_calls: usize,
    contract_exits: usize,
    values: usize,
    borrow_writable_roots: usize,
    borrow_access_segments: usize,
    borrow_argument_accesses: usize,
    borrow_calls: usize,
    borrow_loans: usize,
    borrow_activations: usize,
    borrow_weakenings: usize,
    ownership_segments: usize,
    move_events: usize,
    drop_events: usize,
    operations: usize,
    transitions: usize,
}

impl StateGraphCapacity {
    pub(crate) fn for_program(program: &CheckedTrees) -> Self {
        let mut capacity = Self {
            expressions: ExpressionTableCapacity::default(),
            machines: program.machines().len(),
            contained_machines: program.machine_contained_objects.len(),
            machine_owned_data: program.machine_owned_data.len(),
            states: 0,
            state_parameters: program.state_parameters.len(),
            proof_obligations: program.facts.proof.obligations.len(),
            invariants: program.facts.invariants.definitions.len(),
            contract_fact_refs: program.facts.proof.contract_fact_refs.len(),
            contract_calls: program.facts.proof.contract_calls.len(),
            contract_exits: program.facts.proof.contract_exits.len(),
            values: program.facts.values.values.len(),
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_access_segments: program.facts.borrow.access_segments.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            borrow_loans: program.facts.borrow.loans.len(),
            borrow_activations: program.facts.flow.borrow_activations.len(),
            borrow_weakenings: program.facts.flow.borrow_weakenings.len(),
            ownership_segments: program.facts.flow.ownership_segments.len(),
            move_events: program.facts.flow.moves.len(),
            drop_events: program.facts.flow.drops.len(),
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

    pub(crate) fn for_machine(program: &CheckedTrees, machine: &Machine) -> Self {
        let state_capacity = estimated_machine_segment_capacity(program, machine);
        let statement_capacity = machine_statement_count(program, machine);
        let state_parameter_capacity = program
            .machine_states(machine)
            .iter()
            .map(|state| program.state_parameters(state).len())
            .sum();

        Self {
            expressions: machine_expression_capacity(program, machine),
            machines: 1,
            contained_machines: program.machine_contained_objects(machine).len(),
            machine_owned_data: program.machine_owned_data(machine).len(),
            states: state_capacity,
            state_parameters: state_parameter_capacity,
            proof_obligations: 0,
            invariants: 0,
            contract_fact_refs: machine_contract_fact_ref_count(program, machine),
            contract_calls: machine_contract_call_count(program, machine),
            contract_exits: machine_contract_exit_count(program, machine),
            values: machine_value_count(program, machine),
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_access_segments: program.facts.borrow.access_segments.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            borrow_loans: program.facts.borrow.loans.len(),
            borrow_activations: program.facts.flow.borrow_activations.len(),
            borrow_weakenings: program.facts.flow.borrow_weakenings.len(),
            ownership_segments: program.facts.flow.ownership_segments.len(),
            move_events: program.facts.flow.moves.len(),
            drop_events: program.facts.flow.drops.len(),
            operations: statement_capacity,
            transitions: statement_capacity,
        }
    }

    pub(crate) fn into_state_graph(self) -> StateGraph {
        StateGraph::with_capacity(
            self.expressions,
            self.machines,
            self.contained_machines,
            self.machine_owned_data,
            self.states,
            self.state_parameters,
            self.proof_obligations,
            self.invariants,
            self.contract_fact_refs,
            self.contract_calls,
            self.contract_exits,
            self.values,
            self.borrow_writable_roots,
            self.borrow_access_segments,
            self.borrow_argument_accesses,
            self.borrow_calls,
            self.borrow_loans,
            self.borrow_activations,
            self.borrow_weakenings,
            self.ownership_segments,
            self.move_events,
            self.drop_events,
            self.operations,
            self.transitions,
        )
    }
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
                omega_checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. }
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
