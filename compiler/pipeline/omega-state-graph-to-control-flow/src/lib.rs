use omega_control_flow::{
    ContainedFlow, ControlFlowPlan, InvariantFact, MachineFlow, Operation,
    OperationExpressionRefs, OperationKind, PlannedTransitionTarget, ProofFactKind,
    ProofObligationFact, StateBorrowAccessKind, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowRootKind, StateBorrowSummary, StateBorrowWritableRoot, StateFlow, StateKey,
    StateParameterFlow, TransitionExpressionRefs, TransitionFlow,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{
    ContainedGraph, MachineGraph, StateGraph, StateNode, StateParameterNode, TransitionEdge,
};

pub fn build_control_flow_plan(state_graph: &StateGraph) -> Result<ControlFlowPlan, Diagnostic> {
    let (states, state_parameters) = remap_states(state_graph);

    Ok(ControlFlowPlan {
        expressions: state_graph.expressions.clone(),
        machines: remap_machines(state_graph),
        states,
        state_parameters,
        proof_obligations: remap_proof_obligations(state_graph),
        invariants: remap_invariants(state_graph),
        borrow_writable_roots: remap_borrow_writable_roots(state_graph),
        borrow_argument_accesses: remap_borrow_argument_accesses(state_graph),
        borrow_calls: remap_borrow_calls(state_graph),
        operations: remap_operations(state_graph),
        transitions: remap_transitions(state_graph),
    })
}

fn remap_machines(state_graph: &StateGraph) -> Arena<MachineFlow> {
    let mut machines = Arena::default();

    for (_, machine) in state_graph.machines.iter() {
        machines.append(remap_machine(machine));
    }

    machines
}

fn remap_machine(machine: &MachineGraph) -> MachineFlow {
    MachineFlow {
        symbol: machine.symbol,
        name: machine.name.clone(),
        contains: machine.contains.iter().map(remap_contained).collect(),
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

fn remap_states(state_graph: &StateGraph) -> (Arena<StateFlow>, Arena<StateParameterFlow>) {
    let mut states = Arena::default();
    let mut state_parameters = Arena::default();

    for (_, state) in state_graph.states.iter() {
        states.append(remap_state(state_graph, state, &mut state_parameters));
    }

    (states, state_parameters)
}

fn remap_proof_obligations(state_graph: &StateGraph) -> Arena<ProofObligationFact> {
    let mut obligations = Arena::default();

    for (_, obligation) in state_graph.proof_obligations.iter() {
        obligations.append(ProofObligationFact {
            kind: match obligation.kind {
                omega_state_graph::ProofFactKind::BoundedAssignment => {
                    ProofFactKind::BoundedAssignment
                }
                omega_state_graph::ProofFactKind::BoundedCallArgument => {
                    ProofFactKind::BoundedCallArgument
                }
                omega_state_graph::ProofFactKind::BoundedInitializer => {
                    ProofFactKind::BoundedInitializer
                }
                omega_state_graph::ProofFactKind::BoundedStateReturn => {
                    ProofFactKind::BoundedStateReturn
                }
                omega_state_graph::ProofFactKind::BoundedValue => ProofFactKind::BoundedValue,
                omega_state_graph::ProofFactKind::BoundedTransitionArgument => {
                    ProofFactKind::BoundedTransitionArgument
                }
                omega_state_graph::ProofFactKind::GuardedTransition => {
                    ProofFactKind::GuardedTransition
                }
            },
            machine_symbol: obligation.machine_symbol,
            state_symbol: obligation.state_symbol,
            owner: obligation.owner.clone(),
        });
    }

    obligations
}

fn remap_invariants(state_graph: &StateGraph) -> Arena<InvariantFact> {
    let mut invariants = Arena::default();

    for (_, invariant) in state_graph.invariants.iter() {
        invariants.append(InvariantFact {
            symbol: invariant.symbol,
            name: invariant.name.clone(),
            constraint_count: invariant.constraint_count,
        });
    }

    invariants
}

fn remap_state(
    state_graph: &StateGraph,
    state: &StateNode,
    state_parameters: &mut Arena<StateParameterFlow>,
) -> StateFlow {
    let mut state_flow = StateFlow {
        key: remap_state_key(state.key),
        name: state.name.clone(),
        index: state.index,
        parameters: HandleSpan::empty(),
        borrow: remap_borrow_summary(&state.borrow),
        operations: remap_operation_span(state.operations),
        transitions: remap_transition_span(state.transitions),
    };

    for parameter in state_graph.state_parameters(state) {
        state_parameters.append_to_span(&mut state_flow.parameters, remap_parameter(parameter));
    }

    state_flow
}

fn remap_borrow_writable_roots(state_graph: &StateGraph) -> Arena<StateBorrowWritableRoot> {
    let mut writable_roots = Arena::default();

    for (_, root) in state_graph.borrow_writable_roots.iter() {
        writable_roots.append(StateBorrowWritableRoot {
            symbol: root.symbol,
            name: root.name.clone(),
            kind: match root.kind {
                omega_state_graph::StateBorrowRootKind::OwnedData => StateBorrowRootKind::OwnedData,
                omega_state_graph::StateBorrowRootKind::LocalData => StateBorrowRootKind::LocalData,
                omega_state_graph::StateBorrowRootKind::MutableParameter => {
                    StateBorrowRootKind::MutableParameter
                }
            },
        });
    }

    writable_roots
}

fn remap_borrow_argument_accesses(state_graph: &StateGraph) -> Arena<StateBorrowArgumentAccess> {
    let mut accesses = Arena::default();

    for (_, access) in state_graph.borrow_argument_accesses.iter() {
        accesses.append(StateBorrowArgumentAccess {
            root_name: access.root_name.clone(),
            kind: match access.kind {
                omega_state_graph::StateBorrowAccessKind::Read => StateBorrowAccessKind::Read,
                omega_state_graph::StateBorrowAccessKind::Mutable => {
                    StateBorrowAccessKind::Mutable
                }
            },
        });
    }

    accesses
}

fn remap_borrow_calls(state_graph: &StateGraph) -> Arena<StateBorrowCall> {
    let mut calls = Arena::default();

    for (_, call) in state_graph.borrow_calls.iter() {
        calls.append(StateBorrowCall {
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            receiver_symbol: call.receiver_symbol,
            target_symbol: call.target_symbol,
            receiver: call.receiver.clone(),
            target: call.target.clone(),
            accesses: remap_borrow_argument_access_span(call.accesses),
        });
    }

    calls
}

fn remap_borrow_summary(summary: &omega_state_graph::StateBorrowSummary) -> StateBorrowSummary {
    StateBorrowSummary {
        writable_roots: remap_borrow_writable_root_span(summary.writable_roots),
        mutable_parameter_count: summary.mutable_parameter_count,
        calls: remap_borrow_call_span(summary.calls),
    }
}

fn remap_parameter(parameter: &StateParameterNode) -> StateParameterFlow {
    StateParameterFlow {
        symbol: parameter.symbol,
        name: parameter.name.clone(),
        type_symbol: parameter.type_symbol,
        type_name: parameter.type_name.clone(),
        is_mutable_reference: parameter.is_mutable_reference,
    }
}

fn remap_operations(state_graph: &StateGraph) -> Arena<Operation> {
    let mut operations = Arena::default();

    for (_, operation) in state_graph.operations.iter() {
        operations.append(remap_operation(operation));
    }

    operations
}

fn remap_transitions(state_graph: &StateGraph) -> Arena<TransitionFlow> {
    let mut transitions = Arena::default();

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

fn remap_operation_kind(kind: &omega_state_graph::OperationKind) -> OperationKind {
    match kind {
        omega_state_graph::OperationKind::Assignment => OperationKind::Assignment,
        omega_state_graph::OperationKind::Call {
            receiver_symbol,
            target_symbol,
            receiver,
            target,
        } => OperationKind::Call {
            receiver_symbol: *receiver_symbol,
            target_symbol: *target_symbol,
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
        statement_index: 0,
        target: remap_transition_target(&transition.target),
        continuation: transition.continuation.as_ref().map(remap_transition_target),
        guard: transition.guard.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments: transition.expressions.target_arguments,
            target_value: transition.expressions.target_value,
            continuation_arguments: transition.expressions.continuation_arguments,
            continuation_value: transition.expressions.continuation_value,
            guard: transition.expressions.guard,
        },
    }
}

fn remap_state_span(states: HandleSpan<omega_state_graph::StateNode>) -> HandleSpan<StateFlow> {
    HandleSpan::from_parts(remap_state_handle(states.start()), states.count())
}

fn remap_state_handle(handle: Handle<omega_state_graph::StateNode>) -> Handle<StateFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_operation_span(operations: HandleSpan<omega_state_graph::Operation>) -> HandleSpan<Operation> {
    HandleSpan::from_parts(remap_operation_handle(operations.start()), operations.count())
}

fn remap_operation_handle(
    handle: Handle<omega_state_graph::Operation>,
) -> Handle<Operation> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_transition_span(
    transitions: HandleSpan<omega_state_graph::TransitionEdge>,
) -> HandleSpan<TransitionFlow> {
    HandleSpan::from_parts(remap_transition_handle(transitions.start()), transitions.count())
}

fn remap_borrow_writable_root_span(
    roots: HandleSpan<omega_state_graph::StateBorrowWritableRoot>,
) -> HandleSpan<StateBorrowWritableRoot> {
    HandleSpan::from_parts(remap_borrow_writable_root_handle(roots.start()), roots.count())
}

fn remap_borrow_writable_root_handle(
    handle: Handle<omega_state_graph::StateBorrowWritableRoot>,
) -> Handle<StateBorrowWritableRoot> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_borrow_argument_access_span(
    accesses: HandleSpan<omega_state_graph::StateBorrowArgumentAccess>,
) -> HandleSpan<StateBorrowArgumentAccess> {
    HandleSpan::from_parts(remap_borrow_argument_access_handle(accesses.start()), accesses.count())
}

fn remap_borrow_argument_access_handle(
    handle: Handle<omega_state_graph::StateBorrowArgumentAccess>,
) -> Handle<StateBorrowArgumentAccess> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_borrow_call_span(
    calls: HandleSpan<omega_state_graph::StateBorrowCall>,
) -> HandleSpan<StateBorrowCall> {
    HandleSpan::from_parts(remap_borrow_call_handle(calls.start()), calls.count())
}

fn remap_borrow_call_handle(
    handle: Handle<omega_state_graph::StateBorrowCall>,
) -> Handle<StateBorrowCall> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_transition_handle(
    handle: Handle<omega_state_graph::TransitionEdge>,
) -> Handle<TransitionFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_expression_span(
    expressions: HandleSpan<omega_checked_trees::expression::ExpressionHandle>,
) -> HandleSpan<omega_checked_trees::expression::ExpressionHandle> {
    HandleSpan::from_parts(expressions.start(), expressions.count())
}

fn remap_transition_target(target: &omega_state_graph::PlannedTransitionTarget) -> PlannedTransitionTarget {
    match target {
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
        omega_state_graph::PlannedTransitionTarget::SelfTarget => PlannedTransitionTarget::SelfTarget,
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
