use omega_checked_trees::Program;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::machine::Machine;
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_graph::{
    ContainedGraph, InvariantFact, MachineGraph, Operation, OperationExpressionRefs,
    PlannedTransitionTarget, ProofFactKind, ProofObligationFact, StateBorrowAccessKind,
    StateBorrowArgumentAccess, StateBorrowCall, StateBorrowRootKind, StateBorrowSummary,
    StateBorrowWritableRoot, StateGraph, StateKey, StateNode, TransitionEdge,
    TransitionExpressionRefs,
};
use std::sync::Arc;

mod segments;
mod transitions;

use crate::segments::{segment_has_unconditional_transition, split_state_segments};
use crate::transitions::plan_transition;

pub fn build_state_graph(program: &Program) -> Result<StateGraph, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_state_graph_with_workers(Arc::new(program.clone()), workers.handle())
}

pub fn build_state_graph_with_workers(
    program: Arc<Program>,
    workers: WorkerPoolHandle,
) -> Result<StateGraph, Diagnostic> {
    if program.machines().is_empty() {
        return Ok(StateGraph::default());
    }

    let machine_count = program.machines().len();
    let program_for_machines = Arc::clone(&program);
    let machine_graphs = workers.map_ordered(machine_count, move |index| {
        let machine = program_for_machines
            .machines()
            .get(index)
            .expect("state-graph worker index should be in range");
        let mut local_state_graph = StateGraph::default();
        let machine_graph =
            build_machine_graph(machine, &program_for_machines, &mut local_state_graph)?;

        Ok((local_state_graph, machine_graph))
    });

    let mut state_graph = StateGraph::default();
    for machine_graph in machine_graphs {
        let (local_state_graph, machine_graph) = machine_graph?;
        merge_machine_graph(&mut state_graph, &local_state_graph, &machine_graph);
    }

    state_graph.proof_obligations =
        remap_proof_obligations(program.facts.proof.obligations.iter().map(|(_, fact)| fact));
    state_graph.invariants = remap_invariants(
        program
            .facts
            .invariants
            .definitions
            .iter()
            .map(|(_, fact)| fact),
    );

    Ok(state_graph)
}

fn remap_proof_obligations<'a>(
    facts: impl Iterator<Item = &'a omega_checked_trees::ProofObligationFact>,
) -> omega_core::arena::Arena<ProofObligationFact> {
    let mut obligations = omega_core::arena::Arena::new();

    for fact in facts {
        obligations.append(ProofObligationFact {
            kind: match fact.kind {
                omega_checked_trees::ProofFactKind::BoundedAssignment => {
                    ProofFactKind::BoundedAssignment
                }
                omega_checked_trees::ProofFactKind::BoundedCallArgument => {
                    ProofFactKind::BoundedCallArgument
                }
                omega_checked_trees::ProofFactKind::BoundedInitializer => {
                    ProofFactKind::BoundedInitializer
                }
                omega_checked_trees::ProofFactKind::BoundedStateReturn => {
                    ProofFactKind::BoundedStateReturn
                }
                omega_checked_trees::ProofFactKind::BoundedValue => ProofFactKind::BoundedValue,
                omega_checked_trees::ProofFactKind::BoundedTransitionArgument => {
                    ProofFactKind::BoundedTransitionArgument
                }
                omega_checked_trees::ProofFactKind::GuardedTransition => {
                    ProofFactKind::GuardedTransition
                }
            },
            machine_symbol: fact.machine_symbol,
            state_symbol: fact.state_symbol,
            owner: fact.owner.clone(),
        });
    }

    obligations
}

fn remap_invariants<'a>(
    facts: impl Iterator<Item = &'a omega_checked_trees::InvariantFact>,
) -> omega_core::arena::Arena<InvariantFact> {
    let mut invariants = omega_core::arena::Arena::new();

    for fact in facts {
        invariants.append(InvariantFact {
            symbol: fact.symbol,
            name: fact.name.clone(),
            constraint_count: fact.constraint_count,
        });
    }

    invariants
}

fn merge_machine_graph(target: &mut StateGraph, source: &StateGraph, machine_graph: &MachineGraph) {
    let states = append_remapped_states(target, source, machine_graph.states);

    let contains = target.contained_machines.insert_many(
        source
            .contained_machines
            .span_or_empty(machine_graph.contains)
            .iter()
            .cloned(),
    );

    target.machines.insert(MachineGraph {
        contains,
        states,
        ..machine_graph.clone()
    });
}

fn append_remapped_states(
    target: &mut StateGraph,
    source: &StateGraph,
    states: HandleSpan<StateNode>,
) -> HandleSpan<StateNode> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for state in source.states.span_or_empty(states) {
        let mut parameters = HandleSpan::empty();
        for parameter in source.state_parameters(state) {
            target
                .state_parameters
                .append_to_span(&mut parameters, parameter.clone());
        }

        let operations = append_remapped_operations(target, source, state.operations);
        let transitions = append_remapped_transitions(target, source, state.transitions);
        let borrow = remap_state_borrow_summary(target, source, &state.borrow);
        let handle = target.states.append(StateNode {
            parameters,
            borrow,
            operations,
            transitions,
            ..state.clone()
        });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state-graph state span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn remap_state_borrow_summary(
    target: &mut StateGraph,
    source: &StateGraph,
    borrow: &StateBorrowSummary,
) -> StateBorrowSummary {
    let mut writable_roots = HandleSpan::empty();
    for root in source
        .borrow_writable_roots
        .span_or_empty(borrow.writable_roots)
    {
        target
            .borrow_writable_roots
            .append_to_span(&mut writable_roots, root.clone());
    }

    let calls = append_remapped_borrow_calls(target, source, borrow.calls);

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: borrow.mutable_parameter_count,
        calls,
    }
}

fn append_remapped_borrow_calls(
    target: &mut StateGraph,
    source: &StateGraph,
    calls: HandleSpan<StateBorrowCall>,
) -> HandleSpan<StateBorrowCall> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for call in source.borrow_calls.span_or_empty(calls) {
        let mut accesses = HandleSpan::empty();
        for access in source.borrow_argument_accesses.span_or_empty(call.accesses) {
            target
                .borrow_argument_accesses
                .append_to_span(&mut accesses, access.clone());
        }

        let handle = target.borrow_calls.append(StateBorrowCall {
            accesses,
            ..call.clone()
        });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state-graph borrow call span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_remapped_operations(
    target: &mut StateGraph,
    source: &StateGraph,
    operations: HandleSpan<Operation>,
) -> HandleSpan<Operation> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for operation in source.operations.span_or_empty(operations) {
        let operation = remap_operation(target, source, operation);
        let handle = target.operations.append(operation);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state-graph operation span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_remapped_transitions(
    target: &mut StateGraph,
    source: &StateGraph,
    transitions: HandleSpan<TransitionEdge>,
) -> HandleSpan<TransitionEdge> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for transition in source.transitions.span_or_empty(transitions) {
        let transition = remap_transition(target, source, transition);
        let handle = target.transitions.append(transition);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state-graph transition span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn build_machine_graph(
    machine: &Machine,
    program: &Program,
    state_graph: &mut StateGraph,
) -> Result<MachineGraph, Diagnostic> {
    let machine_symbol = machine.symbol;
    if !machine_symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "machine `{}` has no symbol",
            machine.name
        )));
    }

    let segments = program
        .machine_states(machine)
        .iter()
        .map(|state| split_state_segments(machine, state, program, state_graph))
        .collect::<Vec<_>>();
    let state_indexes = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| (segment.key, index, segment.name.clone()))
        .collect::<Vec<_>>();

    let states = append_machine_states(state_graph, program, &segments, &state_indexes)?;

    Ok(MachineGraph {
        symbol: machine_symbol,
        name: machine.name.clone(),
        contains: machine_contains(state_graph, program, machine),
        states,
    })
}

fn machine_contains(
    state_graph: &mut StateGraph,
    program: &Program,
    machine: &Machine,
) -> HandleSpan<ContainedGraph> {
    let mut contains = HandleSpan::empty();
    for contained in program.machine_contained_objects(machine) {
        state_graph.contained_machines.append_to_span(
            &mut contains,
            ContainedGraph {
                symbol: contained.symbol,
                name: contained.name.clone(),
                type_symbol: contained.type_symbol,
                type_name: contained.type_name.clone(),
            },
        );
    }

    let Some(data_definition) = program
        .data_definitions()
        .iter()
        .find(|data_definition| data_definition.name == machine.name)
    else {
        return contains;
    };

    for member in program.data_members(data_definition) {
        let omega_checked_trees::data::DataMember::Field(field) = member else {
            continue;
        };

        let field_type_name = type_reference_name_handle(program, field.type_reference);
        let Some(target_machine) = program
            .machines()
            .iter()
            .find(|candidate| candidate.name == field_type_name)
        else {
            continue;
        };

        let contained_symbol = program
            .symbols
            .find_child_by_name(machine.symbol, field.name.as_str())
            .unwrap_or(field.symbol);

        if state_graph
            .contained_machines
            .span_or_empty(contains)
            .iter()
            .any(|contained| contained.symbol == contained_symbol)
        {
            continue;
        }

        state_graph.contained_machines.append_to_span(
            &mut contains,
            ContainedGraph {
                symbol: contained_symbol,
                name: field.name.clone(),
                type_symbol: target_machine.symbol,
                type_name: target_machine.name.clone(),
            },
        );
    }

    contains
}

fn type_reference_name_handle(
    program: &Program,
    type_reference: omega_checked_trees::types::TypeReferenceHandle,
) -> omega_checked_trees::name::ProgramName {
    match program.type_reference_table.type_reference(type_reference) {
        omega_checked_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            type_reference_name_handle(program, *referee)
        }
        omega_checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_name_handle(program, *base_type)
        }
        omega_checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Slice { element_type } => {
            type_reference_name_handle(program, *element_type)
        }
        omega_checked_trees::types::TypeReferenceNode::Generic { base_name, .. } => {
            base_name.clone()
        }
        omega_checked_trees::types::TypeReferenceNode::Named { name, .. } => name.clone(),
        omega_checked_trees::types::TypeReferenceNode::Unit => {
            omega_checked_trees::name::ProgramName::default()
        }
    }
}

fn append_machine_states(
    state_graph: &mut StateGraph,
    program: &Program,
    segments: &[Vec<crate::segments::StateSegment>],
    state_indexes: &[(StateKey, usize, omega_checked_trees::name::ProgramName)],
) -> Result<HandleSpan<StateNode>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for (index, segment) in segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
    {
        let mut operations = HandleSpan::empty();
        for (_, operation) in segment.operations.iter() {
            state_graph
                .operations
                .append_to_span(&mut operations, operation.clone());
        }

        let transitions = append_segment_transitions(state_graph, program, segment, state_indexes)?;
        let borrow = state_borrow_summary(state_graph, program, segment.key);
        let handle = state_graph.states.append(StateNode {
            key: segment.key,
            name: segment.name.clone(),
            index,
            parameters: segment.parameters.clone(),
            borrow,
            operations,
            transitions,
        });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("state-graph state span count overflow");
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
}

fn state_borrow_summary(
    state_graph: &mut StateGraph,
    program: &Program,
    key: StateKey,
) -> StateBorrowSummary {
    let Some(state_borrow) = program
        .facts
        .borrow
        .states
        .iter()
        .find(|(_, state_borrow)| {
            state_borrow.machine_symbol == key.machine && state_borrow.state_symbol == key.state
        })
        .map(|(_, state_borrow)| state_borrow)
    else {
        return StateBorrowSummary::default();
    };

    let mut writable_roots = HandleSpan::empty();
    for root in program
        .facts
        .borrow
        .writable_roots
        .span_or_empty(state_borrow.writable_roots)
    {
        state_graph.borrow_writable_roots.append_to_span(
            &mut writable_roots,
            StateBorrowWritableRoot {
                symbol: root.symbol,
                name: root.name.clone(),
                kind: match root.kind {
                    omega_checked_trees::BorrowRootKind::OwnedData => {
                        StateBorrowRootKind::OwnedData
                    }
                    omega_checked_trees::BorrowRootKind::LocalData => {
                        StateBorrowRootKind::LocalData
                    }
                    omega_checked_trees::BorrowRootKind::MutableParameter => {
                        StateBorrowRootKind::MutableParameter
                    }
                },
            },
        );
    }

    let mut calls = HandleSpan::empty();
    for call in program.facts.borrow.calls.span_or_empty(state_borrow.calls) {
        let mut accesses = HandleSpan::empty();
        for access in program
            .facts
            .borrow
            .argument_accesses
            .span_or_empty(call.accesses)
        {
            state_graph.borrow_argument_accesses.append_to_span(
                &mut accesses,
                StateBorrowArgumentAccess {
                    root_name: access.root_name.clone(),
                    kind: match access.kind {
                        omega_checked_trees::BorrowAccessKind::Read => StateBorrowAccessKind::Read,
                        omega_checked_trees::BorrowAccessKind::Mutable => {
                            StateBorrowAccessKind::Mutable
                        }
                    },
                },
            );
        }

        state_graph.borrow_calls.append_to_span(
            &mut calls,
            StateBorrowCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                receiver: call.receiver.clone(),
                target: call.target.clone(),
                accesses,
            },
        );
    }

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: state_borrow.mutable_parameter_count,
        calls,
    }
}

fn append_segment_transitions(
    state_graph: &mut StateGraph,
    program: &Program,
    segment: &crate::segments::StateSegment,
    state_indexes: &[(StateKey, usize, omega_checked_trees::name::ProgramName)],
) -> Result<HandleSpan<TransitionEdge>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for (_, transition) in segment.transitions.iter() {
        let transition =
            plan_transition(segment.key, state_indexes, transition, program, state_graph)?;
        append_transition(state_graph, transition, &mut start, &mut count);
    }

    if let Some(next_segment_name) = &segment.next_segment_name
        && !segment_has_unconditional_transition(segment)
    {
        let next_segment_key = StateKey {
            segment_index: segment.key.segment_index + 1,
            ..segment.key
        };
        let (next_key, next_index) = state_indexes
            .iter()
            .find(|(key, _, _)| *key == next_segment_key)
            .map(|(key, index, _)| (*key, *index))
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "internal state-graph segment `{next_segment_name}` was not indexed"
                ))
            })?;

        append_transition(
            state_graph,
            TransitionEdge {
                target: PlannedTransitionTarget::State {
                    index: next_index,
                    key: next_key,
                    name: next_segment_name.clone(),
                },
                continuation: None,
                expressions: TransitionExpressionRefs::default(),
            },
            &mut start,
            &mut count,
        );
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
}

fn append_transition(
    state_graph: &mut StateGraph,
    transition: TransitionEdge,
    start: &mut Handle<TransitionEdge>,
    count: &mut u32,
) {
    let handle = state_graph.transitions.append(transition);
    if *count == 0 {
        *start = handle;
    }
    *count = count
        .checked_add(1)
        .expect("state-graph transition span count overflow");
}

fn remap_operation(
    target: &mut StateGraph,
    source: &StateGraph,
    operation: &Operation,
) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: operation.kind.clone(),
        expressions: remap_operation_expression_refs(target, source, operation.expressions),
    }
}

fn remap_operation_expression_refs(
    target: &mut StateGraph,
    source: &StateGraph,
    expressions: OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        OperationExpressionRefs::Assignment { target: lhs, value } => {
            OperationExpressionRefs::Assignment {
                target: copy_expression(target, source, lhs),
                value: copy_expression(target, source, value),
            }
        }
        OperationExpressionRefs::Call { arguments } => OperationExpressionRefs::Call {
            arguments: copy_expression_span(target, source, arguments),
        },
        OperationExpressionRefs::Expression(expression) => {
            OperationExpressionRefs::Expression(copy_expression(target, source, expression))
        }
        OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

fn remap_transition(
    target: &mut StateGraph,
    source: &StateGraph,
    transition: &TransitionEdge,
) -> TransitionEdge {
    TransitionEdge {
        target: transition.target.clone(),
        continuation: transition.continuation.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments: copy_expression_span(
                target,
                source,
                transition.expressions.target_arguments,
            ),
            target_value: transition
                .expressions
                .target_value
                .map(|value| copy_expression(target, source, value)),
            continuation_arguments: copy_expression_span(
                target,
                source,
                transition.expressions.continuation_arguments,
            ),
            continuation_value: transition
                .expressions
                .continuation_value
                .map(|value| copy_expression(target, source, value)),
            guard: transition
                .expressions
                .guard
                .map(|guard| copy_expression(target, source, guard)),
        },
    }
}

fn copy_expression(
    target: &mut StateGraph,
    source: &StateGraph,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    target
        .expressions
        .copy_from(&source.expressions, expression)
}

fn copy_expression_span(
    target: &mut StateGraph,
    source: &StateGraph,
    expressions: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    target
        .expressions
        .copy_expression_handles_from(&source.expressions, expressions)
}
