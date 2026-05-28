use omega_control_flow::{
    ContainedFlow, ControlFlowPlan, MachineFlow, MachineOwnedDataFlow, Operation,
    OperationExpressionRefs, OperationKind, PlannedTransitionTarget, StateFlow, StateKey,
    StateParameterFlow, TransitionExpressionRefs, TransitionFlow,
};
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{
    ContainedGraph, MachineGraph, MachineOwnedDataGraph, StateGraph, StateNode, StateParameterNode,
    TransitionEdge,
};

use crate::borrows::{
    remap_borrow_activation_owned, remap_borrow_activations, remap_borrow_argument_access_owned,
    remap_borrow_argument_accesses, remap_borrow_call_owned, remap_borrow_calls,
    remap_borrow_loan_owned, remap_borrow_loans, remap_borrow_summary,
    remap_borrow_weakening_owned, remap_borrow_weakenings, remap_borrow_writable_root_owned,
    remap_borrow_writable_roots,
};
use crate::contracts::{
    remap_contract_call_owned, remap_contract_calls, remap_contract_exit_owned,
    remap_contract_exits, remap_contract_fact_ref_owned, remap_contract_fact_refs,
    remap_contract_summary,
};
use crate::facts::{
    remap_invariant_owned, remap_invariants, remap_proof_obligation_owned, remap_proof_obligations,
};
use crate::handles::{
    remap_contained_span, remap_expression_span, remap_operation_span, remap_owned_data_span,
    remap_parameter_span, remap_state_span, remap_transition_span,
};

pub(crate) fn build_control_flow_plan(
    state_graph: &StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    let (machines, contained_machines, machine_owned_data) = remap_machines(state_graph);
    let (states, state_parameters) = remap_states(state_graph);

    Ok(ControlFlowPlan {
        expressions: state_graph.expressions.clone(),
        machines,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        proof_obligations: remap_proof_obligations(state_graph),
        invariants: remap_invariants(state_graph),
        contract_fact_refs: remap_contract_fact_refs(state_graph),
        contract_calls: remap_contract_calls(state_graph),
        contract_exits: remap_contract_exits(state_graph),
        borrow_writable_roots: remap_borrow_writable_roots(state_graph),
        borrow_access_segments: state_graph.borrow_access_segments.clone(),
        borrow_argument_accesses: remap_borrow_argument_accesses(state_graph),
        borrow_calls: remap_borrow_calls(state_graph),
        borrow_loans: remap_borrow_loans(state_graph),
        borrow_activations: remap_borrow_activations(state_graph),
        borrow_weakenings: remap_borrow_weakenings(state_graph),
        operations: remap_operations(state_graph),
        transitions: remap_transitions(state_graph),
    })
}

pub(crate) fn build_control_flow_plan_owned(
    state_graph: StateGraph,
) -> Result<ControlFlowPlan, Diagnostic> {
    let StateGraph {
        expressions,
        machines,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        proof_obligations,
        invariants,
        contract_fact_refs,
        contract_calls,
        contract_exits,
        borrow_writable_roots,
        borrow_access_segments,
        borrow_argument_accesses,
        borrow_calls,
        borrow_loans,
        borrow_activations,
        borrow_weakenings,
        operations,
        transitions,
    } = state_graph;

    Ok(ControlFlowPlan {
        expressions,
        machines: machines.map(remap_machine_owned),
        contained_machines: contained_machines.map(remap_contained_owned),
        machine_owned_data: machine_owned_data.map(remap_owned_data_owned),
        states: states.map(remap_state_owned),
        state_parameters: state_parameters.map(remap_parameter_owned),
        proof_obligations: proof_obligations.map(remap_proof_obligation_owned),
        invariants: invariants.map(remap_invariant_owned),
        contract_fact_refs: contract_fact_refs.map(remap_contract_fact_ref_owned),
        contract_calls: contract_calls.map(remap_contract_call_owned),
        contract_exits: contract_exits.map(remap_contract_exit_owned),
        borrow_writable_roots: borrow_writable_roots.map(remap_borrow_writable_root_owned),
        borrow_access_segments,
        borrow_argument_accesses: borrow_argument_accesses.map(remap_borrow_argument_access_owned),
        borrow_calls: borrow_calls.map(remap_borrow_call_owned),
        borrow_loans: borrow_loans.map(remap_borrow_loan_owned),
        borrow_activations: borrow_activations.map(remap_borrow_activation_owned),
        borrow_weakenings: borrow_weakenings.map(remap_borrow_weakening_owned),
        operations: operations.map(remap_operation_owned),
        transitions: transitions.map(remap_transition_owned),
    })
}

fn remap_machines(
    state_graph: &StateGraph,
) -> (
    Arena<MachineFlow>,
    Arena<ContainedFlow>,
    Arena<MachineOwnedDataFlow>,
) {
    let mut machines = Arena::with_capacity(state_graph.machines.len());
    let mut contained_machines = Arena::with_capacity(state_graph.contained_machines.len());
    let mut machine_owned_data = Arena::with_capacity(state_graph.machine_owned_data.len());

    for (_, machine) in state_graph.machines.iter() {
        machines.append(remap_machine(
            state_graph,
            machine,
            &mut contained_machines,
            &mut machine_owned_data,
        ));
    }

    (machines, contained_machines, machine_owned_data)
}

fn remap_machine(
    state_graph: &StateGraph,
    machine: &MachineGraph,
    contained_machines: &mut Arena<ContainedFlow>,
    machine_owned_data: &mut Arena<MachineOwnedDataFlow>,
) -> MachineFlow {
    MachineFlow {
        symbol: machine.symbol,
        name: machine.name.clone(),
        attached_data: machine.attached_data.clone(),
        direct_effects: machine.direct_effects,
        reached_effects: machine.reached_effects,
        contains: contained_machines.insert_many(
            state_graph
                .machine_contains(machine)
                .iter()
                .map(remap_contained),
        ),
        owned_data: machine_owned_data.insert_many(
            state_graph
                .machine_owned_data(machine)
                .iter()
                .map(remap_owned_data),
        ),
        states: remap_state_span(machine.states),
    }
}

fn remap_machine_owned(machine: MachineGraph) -> MachineFlow {
    MachineFlow {
        symbol: machine.symbol,
        name: machine.name,
        attached_data: machine.attached_data,
        direct_effects: machine.direct_effects,
        reached_effects: machine.reached_effects,
        contains: remap_contained_span(machine.contains),
        owned_data: remap_owned_data_span(machine.owned_data),
        states: remap_state_span(machine.states),
    }
}

fn remap_contained(contained: &ContainedGraph) -> ContainedFlow {
    ContainedFlow {
        symbol: contained.symbol,
        name: contained.name.clone(),
        type_symbol: contained.type_symbol,
        type_name: contained.type_name.clone(),
    }
}

fn remap_contained_owned(contained: ContainedGraph) -> ContainedFlow {
    ContainedFlow {
        symbol: contained.symbol,
        name: contained.name,
        type_symbol: contained.type_symbol,
        type_name: contained.type_name,
    }
}

fn remap_owned_data(data: &MachineOwnedDataGraph) -> MachineOwnedDataFlow {
    MachineOwnedDataFlow {
        symbol: data.symbol,
        name: data.name.clone(),
        type_reference: data.type_reference,
    }
}

fn remap_owned_data_owned(data: MachineOwnedDataGraph) -> MachineOwnedDataFlow {
    MachineOwnedDataFlow {
        symbol: data.symbol,
        name: data.name,
        type_reference: data.type_reference,
    }
}

fn remap_states(state_graph: &StateGraph) -> (Arena<StateFlow>, Arena<StateParameterFlow>) {
    let mut states = Arena::with_capacity(state_graph.states.len());
    let mut state_parameters = Arena::with_capacity(state_graph.state_parameters.len());

    for (_, state) in state_graph.states.iter() {
        states.append(remap_state(state_graph, state, &mut state_parameters));
    }

    (states, state_parameters)
}

fn remap_state(
    state_graph: &StateGraph,
    state: &StateNode,
    state_parameters: &mut Arena<StateParameterFlow>,
) -> StateFlow {
    StateFlow {
        key: remap_state_key(state.key),
        name: state.name.clone(),
        index: state.index,
        direct_effects: state.direct_effects,
        reached_effects: state.reached_effects,
        parameters: state_parameters.insert_many(
            state_graph
                .state_parameters(state)
                .iter()
                .map(remap_parameter),
        ),
        contracts: remap_contract_summary(&state.contracts),
        borrow: remap_borrow_summary(&state.borrow),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    }
}

fn remap_state_owned(state: StateNode) -> StateFlow {
    StateFlow {
        key: remap_state_key(state.key),
        name: state.name,
        index: state.index,
        direct_effects: state.direct_effects,
        reached_effects: state.reached_effects,
        parameters: remap_parameter_span(state.parameters),
        contracts: remap_contract_summary(&state.contracts),
        borrow: remap_borrow_summary(&state.borrow),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    }
}

fn remap_parameter(parameter: &StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name.clone(),
        type_reference: parameter.type_reference,
        type_symbol: parameter.type_symbol,
        type_name: parameter.type_name.clone(),
        is_mutable_reference: parameter.is_mutable_reference,
    }
}

fn remap_parameter_owned(parameter: StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name,
        type_reference: parameter.type_reference,
        type_symbol: parameter.type_symbol,
        type_name: parameter.type_name,
        is_mutable_reference: parameter.is_mutable_reference,
    }
}

fn remap_operations(state_graph: &StateGraph) -> Arena<Operation> {
    let mut operations = Arena::with_capacity(state_graph.operations.len());

    for (_, operation) in state_graph.operations.iter() {
        operations.append(remap_operation(operation));
    }

    operations
}

fn remap_transitions(state_graph: &StateGraph) -> Arena<TransitionFlow> {
    let mut transitions = Arena::with_capacity(state_graph.transitions.len());

    for (_, transition) in state_graph.transitions.iter() {
        transitions.append(remap_transition(transition));
    }

    transitions
}

fn remap_operation(operation: &omega_state_graph::Operation) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: remap_operation_kind(&operation.kind),
        expressions: remap_operation_expression_refs(operation.expressions),
    }
}

fn remap_operation_owned(operation: omega_state_graph::Operation) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: remap_operation_kind_owned(operation.kind),
        expressions: remap_operation_expression_refs(operation.expressions),
    }
}

fn remap_operation_kind(kind: &omega_state_graph::OperationKind) -> OperationKind {
    match kind {
        omega_state_graph::OperationKind::Assignment => OperationKind::Assignment,
        omega_state_graph::OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver,
            receiver,
            target,
        } => OperationKind::Call {
            receiver_symbol: *receiver_symbol,
            target_symbol: *target_symbol,
            has_receiver: *has_receiver,
            receiver: receiver.clone(),
            target: target.clone(),
        },
        omega_state_graph::OperationKind::ConstantIntegerAssignment => {
            OperationKind::ConstantIntegerAssignment
        }
        omega_state_graph::OperationKind::Expression => OperationKind::Expression,
        omega_state_graph::OperationKind::LocalData => OperationKind::LocalData,
        omega_state_graph::OperationKind::StaticAssignment => OperationKind::StaticAssignment,
    }
}

fn remap_operation_kind_owned(kind: omega_state_graph::OperationKind) -> OperationKind {
    match kind {
        omega_state_graph::OperationKind::Assignment => OperationKind::Assignment,
        omega_state_graph::OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver,
            receiver,
            target,
        } => OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver,
            receiver,
            target,
        },
        omega_state_graph::OperationKind::ConstantIntegerAssignment => {
            OperationKind::ConstantIntegerAssignment
        }
        omega_state_graph::OperationKind::Expression => OperationKind::Expression,
        omega_state_graph::OperationKind::LocalData => OperationKind::LocalData,
        omega_state_graph::OperationKind::StaticAssignment => OperationKind::StaticAssignment,
    }
}

fn remap_operation_expression_refs(
    expressions: omega_state_graph::OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        omega_state_graph::OperationExpressionRefs::Assignment { target, value } => {
            OperationExpressionRefs::Assignment { target, value }
        }
        omega_state_graph::OperationExpressionRefs::Call { arguments } => {
            OperationExpressionRefs::Call {
                arguments: remap_expression_span(arguments),
            }
        }
        omega_state_graph::OperationExpressionRefs::Expression(expression) => {
            OperationExpressionRefs::Expression(expression)
        }
        omega_state_graph::OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

fn remap_transition(transition: &TransitionEdge) -> TransitionFlow {
    TransitionFlow {
        statement_index: transition.statement_index,
        target: remap_transition_target(&transition.target),
        continuation: remap_transition_target(&transition.continuation),
        expressions: TransitionExpressionRefs {
            target_arguments: transition.expressions.target_arguments,
            target_value: transition.expressions.target_value,
            continuation_arguments: transition.expressions.continuation_arguments,
            continuation_value: transition.expressions.continuation_value,
            guard: transition.expressions.guard,
        },
    }
}

fn remap_transition_owned(transition: TransitionEdge) -> TransitionFlow {
    TransitionFlow {
        statement_index: transition.statement_index,
        target: remap_transition_target_owned(transition.target),
        continuation: remap_transition_target_owned(transition.continuation),
        expressions: TransitionExpressionRefs {
            target_arguments: transition.expressions.target_arguments,
            target_value: transition.expressions.target_value,
            continuation_arguments: transition.expressions.continuation_arguments,
            continuation_value: transition.expressions.continuation_value,
            guard: transition.expressions.guard,
        },
    }
}

fn remap_transition_target(
    target: &omega_state_graph::PlannedTransitionTarget,
) -> PlannedTransitionTarget {
    match target {
        omega_state_graph::PlannedTransitionTarget::None => PlannedTransitionTarget::None,
        omega_state_graph::PlannedTransitionTarget::State { index, key, name } => {
            PlannedTransitionTarget::State {
                index: *index,
                key: remap_state_key(*key),
                name: name.clone(),
            }
        }
        omega_state_graph::PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
        } => PlannedTransitionTarget::Nested {
            receiver_symbol: *receiver_symbol,
            state_symbol: *state_symbol,
            receiver: receiver.clone(),
            state: state.clone(),
        },
        omega_state_graph::PlannedTransitionTarget::SelfTarget => {
            PlannedTransitionTarget::SelfTarget
        }
        omega_state_graph::PlannedTransitionTarget::Terminal => PlannedTransitionTarget::Terminal,
    }
}

fn remap_transition_target_owned(
    target: omega_state_graph::PlannedTransitionTarget,
) -> PlannedTransitionTarget {
    match target {
        omega_state_graph::PlannedTransitionTarget::None => PlannedTransitionTarget::None,
        omega_state_graph::PlannedTransitionTarget::State { index, key, name } => {
            PlannedTransitionTarget::State {
                index,
                key: remap_state_key(key),
                name,
            }
        }
        omega_state_graph::PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
        } => PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
        },
        omega_state_graph::PlannedTransitionTarget::SelfTarget => {
            PlannedTransitionTarget::SelfTarget
        }
        omega_state_graph::PlannedTransitionTarget::Terminal => PlannedTransitionTarget::Terminal,
    }
}

fn remap_state_key(key: omega_state_graph::StateKey) -> StateKey {
    StateKey {
        machine: key.machine,
        state: key.state,
        segment_index: key.segment_index,
    }
}
