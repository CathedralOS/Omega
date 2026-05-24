use omega_control_flow::{
    ContainedFlow, ControlFlowPlan, InvariantFact, MachineFlow, MachineOwnedDataFlow, Operation,
    OperationExpressionRefs, OperationKind, PlannedTransitionTarget, ProofFactKind,
    ProofObligationFact, ProofObligationOwner, StateBorrowAccessKind, StateBorrowArgumentAccess,
    StateBorrowCall, StateBorrowRootKind, StateBorrowSummary, StateBorrowWritableRoot,
    StateContractCall, StateContractExit, StateContractFactKind, StateContractFactRef,
    StateContractSummary, StateFlow, StateKey, StateParameterFlow, TransitionExpressionRefs,
    TransitionFlow,
};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_state_graph::{
    ContainedGraph, MachineGraph, MachineOwnedDataGraph, StateGraph, StateNode, StateParameterNode,
    TransitionEdge,
};

pub fn build_control_flow_plan(state_graph: &StateGraph) -> Result<ControlFlowPlan, Diagnostic> {
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
        operations: remap_operations(state_graph),
        transitions: remap_transitions(state_graph),
    })
}

pub fn build_control_flow_plan_owned(
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

fn remap_proof_obligations(state_graph: &StateGraph) -> Arena<ProofObligationFact> {
    let mut obligations = Arena::with_capacity(state_graph.proof_obligations.len());

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
            owner: remap_proof_owner(&obligation.owner),
        });
    }

    obligations
}

fn remap_proof_obligation_owned(
    obligation: omega_state_graph::ProofObligationFact,
) -> ProofObligationFact {
    ProofObligationFact {
        kind: match obligation.kind {
            omega_state_graph::ProofFactKind::BoundedAssignment => ProofFactKind::BoundedAssignment,
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
            omega_state_graph::ProofFactKind::GuardedTransition => ProofFactKind::GuardedTransition,
        },
        machine_symbol: obligation.machine_symbol,
        state_symbol: obligation.state_symbol,
        owner: remap_proof_owner(&obligation.owner),
    }
}

fn remap_proof_owner(owner: &omega_state_graph::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        omega_state_graph::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        omega_state_graph::ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::MachineState {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_state_graph::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        omega_state_graph::ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_state_graph::ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_state_graph::ProofObligationOwner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol,
            parameter_symbol,
        } => ProofObligationOwner::CallParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            target_symbol: *target_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_state_graph::ProofObligationOwner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::TransitionParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
    }
}

fn remap_invariants(state_graph: &StateGraph) -> Arena<InvariantFact> {
    let mut invariants = Arena::with_capacity(state_graph.invariants.len());

    for (_, invariant) in state_graph.invariants.iter() {
        invariants.append(InvariantFact {
            symbol: invariant.symbol,
            name: invariant.name.clone(),
            constraint_count: invariant.constraint_count,
        });
    }

    invariants
}

fn remap_invariant_owned(invariant: omega_state_graph::InvariantFact) -> InvariantFact {
    InvariantFact {
        symbol: invariant.symbol,
        name: invariant.name,
        constraint_count: invariant.constraint_count,
    }
}

fn remap_contract_fact_refs(state_graph: &StateGraph) -> Arena<StateContractFactRef> {
    let mut refs = Arena::with_capacity(state_graph.contract_fact_refs.len());

    for (_, reference) in state_graph.contract_fact_refs.iter() {
        refs.append(remap_contract_fact_ref(reference));
    }

    refs
}

fn remap_contract_fact_ref(
    reference: &omega_state_graph::StateContractFactRef,
) -> StateContractFactRef {
    StateContractFactRef {
        kind: match reference.kind {
            omega_state_graph::StateContractFactKind::Requires => StateContractFactKind::Requires,
            omega_state_graph::StateContractFactKind::Ensures => StateContractFactKind::Ensures,
            omega_state_graph::StateContractFactKind::Trusted => StateContractFactKind::Trusted,
        },
        fact: reference.fact,
    }
}

fn remap_contract_fact_ref_owned(
    reference: omega_state_graph::StateContractFactRef,
) -> StateContractFactRef {
    remap_contract_fact_ref(&reference)
}

fn remap_contract_calls(state_graph: &StateGraph) -> Arena<StateContractCall> {
    let mut calls = Arena::with_capacity(state_graph.contract_calls.len());

    for (_, call) in state_graph.contract_calls.iter() {
        calls.append(remap_contract_call(call));
    }

    calls
}

fn remap_contract_call(call: &omega_state_graph::StateContractCall) -> StateContractCall {
    StateContractCall {
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        target_machine_symbol: call.target_machine_symbol,
        target_state_symbol: call.target_state_symbol,
        requires: remap_contract_fact_ref_span(call.requires),
        ensures: remap_contract_fact_ref_span(call.ensures),
    }
}

fn remap_contract_call_owned(call: omega_state_graph::StateContractCall) -> StateContractCall {
    remap_contract_call(&call)
}

fn remap_contract_exits(state_graph: &StateGraph) -> Arena<StateContractExit> {
    let mut exits = Arena::with_capacity(state_graph.contract_exits.len());

    for (_, exit) in state_graph.contract_exits.iter() {
        exits.append(remap_contract_exit(exit));
    }

    exits
}

fn remap_contract_exit(exit: &omega_state_graph::StateContractExit) -> StateContractExit {
    StateContractExit {
        statement_index: exit.statement_index,
        ensures: remap_contract_fact_ref_span(exit.ensures),
    }
}

fn remap_contract_exit_owned(exit: omega_state_graph::StateContractExit) -> StateContractExit {
    remap_contract_exit(&exit)
}

fn remap_contract_summary(
    contracts: &omega_state_graph::StateContractSummary,
) -> StateContractSummary {
    StateContractSummary {
        calls: remap_contract_call_span(contracts.calls),
        exits: remap_contract_exit_span(contracts.exits),
    }
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

fn remap_borrow_writable_roots(state_graph: &StateGraph) -> Arena<StateBorrowWritableRoot> {
    let mut writable_roots = Arena::with_capacity(state_graph.borrow_writable_roots.len());

    for (_, root) in state_graph.borrow_writable_roots.iter() {
        writable_roots.append(StateBorrowWritableRoot {
            symbol: root.symbol,
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

fn remap_borrow_writable_root_owned(
    root: omega_state_graph::StateBorrowWritableRoot,
) -> StateBorrowWritableRoot {
    StateBorrowWritableRoot {
        symbol: root.symbol,
        kind: match root.kind {
            omega_state_graph::StateBorrowRootKind::OwnedData => StateBorrowRootKind::OwnedData,
            omega_state_graph::StateBorrowRootKind::LocalData => StateBorrowRootKind::LocalData,
            omega_state_graph::StateBorrowRootKind::MutableParameter => {
                StateBorrowRootKind::MutableParameter
            }
        },
    }
}

fn remap_borrow_argument_accesses(state_graph: &StateGraph) -> Arena<StateBorrowArgumentAccess> {
    let mut accesses = Arena::with_capacity(state_graph.borrow_argument_accesses.len());

    for (_, access) in state_graph.borrow_argument_accesses.iter() {
        accesses.append(StateBorrowArgumentAccess {
            root_symbol: access.root_symbol,
            segments: access.segments,
            kind: match access.kind {
                omega_state_graph::StateBorrowAccessKind::Read => StateBorrowAccessKind::Read,
                omega_state_graph::StateBorrowAccessKind::Mutable => StateBorrowAccessKind::Mutable,
            },
        });
    }

    accesses
}

fn remap_borrow_argument_access_owned(
    access: omega_state_graph::StateBorrowArgumentAccess,
) -> StateBorrowArgumentAccess {
    StateBorrowArgumentAccess {
        root_symbol: access.root_symbol,
        segments: access.segments,
        kind: match access.kind {
            omega_state_graph::StateBorrowAccessKind::Read => StateBorrowAccessKind::Read,
            omega_state_graph::StateBorrowAccessKind::Mutable => StateBorrowAccessKind::Mutable,
        },
    }
}

fn remap_borrow_calls(state_graph: &StateGraph) -> Arena<StateBorrowCall> {
    let mut calls = Arena::with_capacity(state_graph.borrow_calls.len());

    for (_, call) in state_graph.borrow_calls.iter() {
        calls.append(StateBorrowCall {
            statement_index: call.statement_index,
            call_ordinal: call.call_ordinal,
            receiver_symbol: call.receiver_symbol,
            target_symbol: call.target_symbol,
            has_receiver: call.has_receiver,
            accesses: remap_borrow_argument_access_span(call.accesses),
        });
    }

    calls
}

fn remap_borrow_call_owned(call: omega_state_graph::StateBorrowCall) -> StateBorrowCall {
    StateBorrowCall {
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        receiver_symbol: call.receiver_symbol,
        target_symbol: call.target_symbol,
        has_receiver: call.has_receiver,
        accesses: remap_borrow_argument_access_span(call.accesses),
    }
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

fn remap_contained_span(
    contained: HandleSpan<omega_state_graph::ContainedGraph>,
) -> HandleSpan<ContainedFlow> {
    HandleSpan::from_parts(remap_contained_handle(contained.start()), contained.count())
}

fn remap_contained_handle(
    handle: Handle<omega_state_graph::ContainedGraph>,
) -> Handle<ContainedFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_owned_data_span(
    owned_data: HandleSpan<omega_state_graph::MachineOwnedDataGraph>,
) -> HandleSpan<MachineOwnedDataFlow> {
    HandleSpan::from_parts(
        remap_owned_data_handle(owned_data.start()),
        owned_data.count(),
    )
}

fn remap_owned_data_handle(
    handle: Handle<omega_state_graph::MachineOwnedDataGraph>,
) -> Handle<MachineOwnedDataFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_parameter_span(
    parameters: HandleSpan<omega_state_graph::StateParameterNode>,
) -> HandleSpan<StateParameterFlow> {
    HandleSpan::from_parts(
        remap_parameter_handle(parameters.start()),
        parameters.count(),
    )
}

fn remap_parameter_handle(
    handle: Handle<omega_state_graph::StateParameterNode>,
) -> Handle<StateParameterFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_state_span(states: HandleSpan<omega_state_graph::StateNode>) -> HandleSpan<StateFlow> {
    HandleSpan::from_parts(remap_state_handle(states.start()), states.count())
}

fn remap_state_handle(handle: Handle<omega_state_graph::StateNode>) -> Handle<StateFlow> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_operation_span(
    operations: HandleSpan<omega_state_graph::Operation>,
) -> HandleSpan<Operation> {
    HandleSpan::from_parts(
        remap_operation_handle(operations.start()),
        operations.count(),
    )
}

fn remap_operation_handle(handle: Handle<omega_state_graph::Operation>) -> Handle<Operation> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_transition_span(
    transitions: HandleSpan<omega_state_graph::TransitionEdge>,
) -> HandleSpan<TransitionFlow> {
    HandleSpan::from_parts(
        remap_transition_handle(transitions.start()),
        transitions.count(),
    )
}

fn remap_borrow_writable_root_span(
    roots: HandleSpan<omega_state_graph::StateBorrowWritableRoot>,
) -> HandleSpan<StateBorrowWritableRoot> {
    HandleSpan::from_parts(
        remap_borrow_writable_root_handle(roots.start()),
        roots.count(),
    )
}

fn remap_borrow_writable_root_handle(
    handle: Handle<omega_state_graph::StateBorrowWritableRoot>,
) -> Handle<StateBorrowWritableRoot> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_borrow_argument_access_span(
    accesses: HandleSpan<omega_state_graph::StateBorrowArgumentAccess>,
) -> HandleSpan<StateBorrowArgumentAccess> {
    HandleSpan::from_parts(
        remap_borrow_argument_access_handle(accesses.start()),
        accesses.count(),
    )
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

fn remap_contract_fact_ref_span(
    refs: HandleSpan<omega_state_graph::StateContractFactRef>,
) -> HandleSpan<StateContractFactRef> {
    HandleSpan::from_parts(remap_contract_fact_ref_handle(refs.start()), refs.count())
}

fn remap_contract_fact_ref_handle(
    handle: Handle<omega_state_graph::StateContractFactRef>,
) -> Handle<StateContractFactRef> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_contract_call_span(
    calls: HandleSpan<omega_state_graph::StateContractCall>,
) -> HandleSpan<StateContractCall> {
    HandleSpan::from_parts(remap_contract_call_handle(calls.start()), calls.count())
}

fn remap_contract_call_handle(
    handle: Handle<omega_state_graph::StateContractCall>,
) -> Handle<StateContractCall> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn remap_contract_exit_span(
    exits: HandleSpan<omega_state_graph::StateContractExit>,
) -> HandleSpan<StateContractExit> {
    HandleSpan::from_parts(remap_contract_exit_handle(exits.start()), exits.count())
}

fn remap_contract_exit_handle(
    handle: Handle<omega_state_graph::StateContractExit>,
) -> Handle<StateContractExit> {
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
