use omega_checked_trees::Program;
use omega_checked_trees::expression::ExpressionHandle;
use omega_checked_trees::machine::Machine;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_state_graph::{
    ContainedGraph, InvariantFact, MachineGraph, MachineOwnedDataGraph, Operation,
    OperationExpressionRefs, PlannedTransitionTarget, ProofFactKind, ProofObligationFact,
    ProofObligationOwner, StateBorrowAccessKind, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowRootKind, StateBorrowSummary, StateBorrowWritableRoot, StateGraph, StateKey,
    StateNode, StateParameterNode, TransitionEdge, TransitionExpressionRefs,
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

pub fn build_state_graph_owned(program: Program) -> Result<StateGraph, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_state_graph_with_workers(Arc::new(program), workers.handle())
}

pub fn build_state_graph_with_workers(
    program: Arc<Program>,
    workers: WorkerPoolHandle,
) -> Result<StateGraph, Diagnostic> {
    if program.machines().is_empty() {
        return Ok(StateGraph::with_capacity(
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ));
    }

    let machine_count = program.machines().len();
    let graph_capacity = StateGraphCapacity::for_program(&program);
    let program_for_machines = Arc::clone(&program);
    let machine_graphs = workers.map_ordered(machine_count, move |index| {
        let machine = program_for_machines
            .machines()
            .get(index)
            .expect("state-graph worker index should be in range");
        let mut local_state_graph =
            StateGraphCapacity::for_machine(&program_for_machines, machine).into_state_graph();
        let machine_graph =
            build_machine_graph(machine, &program_for_machines, &mut local_state_graph)?;

        Ok((local_state_graph, machine_graph))
    });

    let mut state_graph = graph_capacity.into_state_graph();
    for machine_graph in machine_graphs {
        let (local_state_graph, machine_graph) = machine_graph?;
        merge_machine_graph(&mut state_graph, local_state_graph, machine_graph);
    }

    state_graph.proof_obligations = remap_proof_obligations(
        program.facts.proof.obligations.len(),
        program.facts.proof.obligations.iter().map(|(_, fact)| fact),
    );
    state_graph.invariants = remap_invariants(
        program.facts.invariants.definitions.len(),
        program
            .facts
            .invariants
            .definitions
            .iter()
            .map(|(_, fact)| fact),
    );

    Ok(state_graph)
}

#[derive(Debug, Clone, Copy, Default)]
struct StateGraphCapacity {
    machines: usize,
    contained_machines: usize,
    machine_owned_data: usize,
    states: usize,
    state_parameters: usize,
    proof_obligations: usize,
    invariants: usize,
    borrow_writable_roots: usize,
    borrow_argument_accesses: usize,
    borrow_calls: usize,
    operations: usize,
    transitions: usize,
}

impl StateGraphCapacity {
    fn for_program(program: &Program) -> Self {
        let mut capacity = Self {
            machines: program.machines().len(),
            contained_machines: program.machine_contained_objects.len(),
            machine_owned_data: program.machine_owned_data.len(),
            states: 0,
            state_parameters: program.state_parameters.len(),
            proof_obligations: program.facts.proof.obligations.len(),
            invariants: program.facts.invariants.definitions.len(),
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            operations: program.statement_table.statement_count(),
            transitions: program.statement_table.transition_target_count(),
        };

        for machine in program.machines() {
            capacity.states = capacity
                .states
                .saturating_add(estimated_machine_segment_capacity(program, machine));
        }

        capacity
    }

    fn for_machine(program: &Program, machine: &Machine) -> Self {
        let state_capacity = estimated_machine_segment_capacity(program, machine);
        let statement_capacity = machine_statement_count(program, machine);
        let state_parameter_capacity = program
            .machine_states(machine)
            .iter()
            .map(|state| program.state_parameters(state).len())
            .sum();

        Self {
            machines: 1,
            contained_machines: program.machine_contained_objects(machine).len(),
            machine_owned_data: program.machine_owned_data(machine).len(),
            states: state_capacity,
            state_parameters: state_parameter_capacity,
            proof_obligations: 0,
            invariants: 0,
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            operations: statement_capacity,
            transitions: statement_capacity,
        }
    }

    fn into_state_graph(self) -> StateGraph {
        StateGraph::with_capacity(
            self.machines,
            self.contained_machines,
            self.machine_owned_data,
            self.states,
            self.state_parameters,
            self.proof_obligations,
            self.invariants,
            self.borrow_writable_roots,
            self.borrow_argument_accesses,
            self.borrow_calls,
            self.operations,
            self.transitions,
        )
    }
}

fn machine_statement_count(program: &Program, machine: &Machine) -> usize {
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

fn remap_proof_obligations<'a>(
    fact_count: usize,
    facts: impl Iterator<Item = &'a omega_checked_trees::ProofObligationFact>,
) -> omega_core::arena::Arena<ProofObligationFact> {
    let mut obligations = omega_core::arena::Arena::with_capacity(fact_count);

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
            owner: remap_proof_owner(&fact.owner),
        });
    }

    obligations
}

fn remap_proof_owner(owner: &omega_checked_trees::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        omega_checked_trees::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        omega_checked_trees::ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::MachineState {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_checked_trees::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        omega_checked_trees::ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_checked_trees::ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
        omega_checked_trees::ProofObligationOwner::CallParameter {
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
        omega_checked_trees::ProofObligationOwner::TransitionParameter {
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

fn remap_invariants<'a>(
    fact_count: usize,
    facts: impl Iterator<Item = &'a omega_checked_trees::InvariantFact>,
) -> omega_core::arena::Arena<InvariantFact> {
    let mut invariants = omega_core::arena::Arena::with_capacity(fact_count);

    for fact in facts {
        invariants.append(InvariantFact {
            symbol: fact.symbol,
            name: fact.name.clone(),
            constraint_count: fact.constraint_count,
        });
    }

    invariants
}

fn merge_machine_graph(target: &mut StateGraph, source: StateGraph, machine_graph: MachineGraph) {
    let StateGraph {
        expressions,
        machines: _,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        proof_obligations: _,
        invariants: _,
        borrow_writable_roots,
        borrow_argument_accesses,
        borrow_calls,
        operations,
        transitions,
    } = source;

    let states = append_remapped_states(
        target,
        &expressions,
        states.into_span_items(machine_graph.states),
        &state_parameters,
        &borrow_writable_roots,
        &borrow_argument_accesses,
        &borrow_calls,
        &operations,
        &transitions,
    );

    let contains = target
        .contained_machines
        .insert_many(contained_machines.into_span_items(machine_graph.contains));

    let owned_data = target
        .machine_owned_data
        .insert_many(machine_owned_data.into_span_items(machine_graph.owned_data));

    target.machines.insert(MachineGraph {
        symbol: machine_graph.symbol,
        name: machine_graph.name,
        contains,
        owned_data,
        states,
    });
}

fn append_remapped_states(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    states: impl Iterator<Item = StateNode>,
    source_state_parameters: &Arena<StateParameterNode>,
    source_borrow_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_borrow_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_borrow_calls: &Arena<StateBorrowCall>,
    source_operations: &Arena<Operation>,
    source_transitions: &Arena<TransitionEdge>,
) -> HandleSpan<StateNode> {
    let mut remapped_states = HandleSpan::empty();

    for state in states {
        let parameters = target.state_parameters.insert_many(
            source_state_parameters
                .span_or_empty(state.parameters)
                .iter()
                .cloned(),
        );

        let operations = append_remapped_operations(
            target,
            source_expressions,
            source_operations,
            state.operations,
        );
        let transitions = append_remapped_transitions(
            target,
            source_expressions,
            source_transitions,
            state.transitions,
        );
        let borrow = remap_state_borrow_summary(
            target,
            source_borrow_writable_roots,
            source_borrow_argument_accesses,
            source_borrow_calls,
            &state.borrow,
        );
        target.states.append_to_span(
            &mut remapped_states,
            StateNode {
                key: state.key,
                name: state.name,
                index: state.index,
                parameters,
                borrow,
                operations,
                transitions,
            },
        );
    }

    remapped_states
}

fn remap_state_borrow_summary(
    target: &mut StateGraph,
    source_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_calls: &Arena<StateBorrowCall>,
    borrow: &StateBorrowSummary,
) -> StateBorrowSummary {
    let writable_roots = target.borrow_writable_roots.insert_many(
        source_writable_roots
            .span_or_empty(borrow.writable_roots)
            .iter()
            .cloned(),
    );

    let calls =
        append_remapped_borrow_calls(target, source_argument_accesses, source_calls, borrow.calls);

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: borrow.mutable_parameter_count,
        calls,
    }
}

fn append_remapped_borrow_calls(
    target: &mut StateGraph,
    source_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_calls: &Arena<StateBorrowCall>,
    calls: HandleSpan<StateBorrowCall>,
) -> HandleSpan<StateBorrowCall> {
    let mut remapped_calls = HandleSpan::empty();

    for call in source_calls.span_or_empty(calls) {
        let accesses = target.borrow_argument_accesses.insert_many(
            source_argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .cloned(),
        );

        target.borrow_calls.append_to_span(
            &mut remapped_calls,
            StateBorrowCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                has_receiver: call.has_receiver,
                accesses,
            },
        );
    }

    remapped_calls
}

fn append_remapped_operations(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    source_operations: &Arena<Operation>,
    operations: HandleSpan<Operation>,
) -> HandleSpan<Operation> {
    let mut remapped_operations = HandleSpan::empty();

    for operation in source_operations.span_or_empty(operations) {
        let operation = remap_operation(target, source_expressions, operation);
        target
            .operations
            .append_to_span(&mut remapped_operations, operation);
    }

    remapped_operations
}

fn append_remapped_transitions(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    source_transitions: &Arena<TransitionEdge>,
    transitions: HandleSpan<TransitionEdge>,
) -> HandleSpan<TransitionEdge> {
    let mut remapped_transitions = HandleSpan::empty();

    for transition in source_transitions.span_or_empty(transitions) {
        let transition = remap_transition(target, source_expressions, transition);
        target
            .transitions
            .append_to_span(&mut remapped_transitions, transition);
    }

    remapped_transitions
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

    let mut segments = Vec::with_capacity(estimated_machine_segment_capacity(program, machine));
    let mut segment_transitions =
        omega_core::arena::Arena::with_capacity(machine_statement_count(program, machine));
    for state in program.machine_states(machine) {
        split_state_segments(
            machine,
            state,
            program,
            state_graph,
            &mut segment_transitions,
            &mut segments,
        );
    }

    let states = append_machine_states(state_graph, program, &segments, &segment_transitions)?;

    Ok(MachineGraph {
        symbol: machine_symbol,
        name: machine.name.clone(),
        contains: machine_contains(state_graph, program, machine),
        owned_data: machine_owned_data(state_graph, program, machine),
        states,
    })
}

fn machine_owned_data(
    state_graph: &mut StateGraph,
    program: &Program,
    machine: &Machine,
) -> HandleSpan<MachineOwnedDataGraph> {
    state_graph
        .machine_owned_data
        .insert_many(
            program
                .machine_owned_data(machine)
                .iter()
                .map(|data| MachineOwnedDataGraph {
                    symbol: data.symbol,
                    name: data.name.clone(),
                    type_reference: data.type_reference,
                }),
        )
}

fn estimated_machine_segment_capacity(program: &Program, machine: &Machine) -> usize {
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
    segments: &[crate::segments::StateSegment],
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
) -> Result<HandleSpan<StateNode>, Diagnostic> {
    let mut states = HandleSpan::empty();

    for (index, segment) in segments.iter().enumerate() {
        let transitions = append_segment_transitions(
            state_graph,
            program,
            segment,
            segments,
            segment_transitions,
        )?;
        let borrow = state_borrow_summary(state_graph, program, segment.key);
        state_graph.states.append_to_span(
            &mut states,
            StateNode {
                key: segment.key,
                name: segment.name.clone(),
                index,
                parameters: segment.parameters,
                borrow,
                operations: segment.operations,
                transitions,
            },
        );
    }

    Ok(states)
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

    let writable_roots = state_graph.borrow_writable_roots.insert_many(
        program
            .facts
            .borrow
            .writable_roots
            .span_or_empty(state_borrow.writable_roots)
            .iter()
            .map(|root| StateBorrowWritableRoot {
                symbol: root.symbol,
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
            }),
    );

    let mut calls = HandleSpan::empty();
    for call in program.facts.borrow.calls.span_or_empty(state_borrow.calls) {
        let accesses = state_graph.borrow_argument_accesses.insert_many(
            program
                .facts
                .borrow
                .argument_accesses
                .span_or_empty(call.accesses)
                .iter()
                .map(|access| StateBorrowArgumentAccess {
                    root_symbol: access.root_symbol,
                    kind: match access.kind {
                        omega_checked_trees::BorrowAccessKind::Read => StateBorrowAccessKind::Read,
                        omega_checked_trees::BorrowAccessKind::Mutable => {
                            StateBorrowAccessKind::Mutable
                        }
                    },
                }),
        );

        state_graph.borrow_calls.append_to_span(
            &mut calls,
            StateBorrowCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                receiver_symbol: call.receiver_symbol,
                target_symbol: call.target_symbol,
                has_receiver: call.has_receiver,
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
    segments: &[crate::segments::StateSegment],
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
) -> Result<HandleSpan<TransitionEdge>, Diagnostic> {
    let mut transitions = HandleSpan::empty();

    for transition in segment_transitions.span_or_empty(segment.transitions) {
        let transition = plan_transition(segment.key, segments, transition, program, state_graph)?;
        state_graph
            .transitions
            .append_to_span(&mut transitions, transition);
    }

    if segment.next_segment_key.is_valid()
        && !segment_has_unconditional_transition(segment, segment_transitions)
    {
        let next_segment_key = segment.next_segment_key;
        let (next_index, next_segment) = segments
            .iter()
            .enumerate()
            .find(|(_, segment)| segment.key == next_segment_key)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "internal state-graph segment #{} was not indexed",
                    next_segment_key.segment_index
                ))
            })?;

        state_graph.transitions.append_to_span(
            &mut transitions,
            TransitionEdge {
                statement_index: 0,
                target: PlannedTransitionTarget::State {
                    index: next_index,
                    key: next_segment.key,
                    name: next_segment.name.clone(),
                },
                continuation: PlannedTransitionTarget::None,
                expressions: TransitionExpressionRefs::default(),
            },
        );
    }

    Ok(transitions)
}

fn remap_operation(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    operation: &Operation,
) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: operation.kind.clone(),
        expressions: remap_operation_expression_refs(
            target,
            source_expressions,
            operation.expressions,
        ),
    }
}

fn remap_operation_expression_refs(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    expressions: OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        OperationExpressionRefs::Assignment { target: lhs, value } => {
            OperationExpressionRefs::Assignment {
                target: copy_expression(target, source_expressions, lhs),
                value: copy_expression(target, source_expressions, value),
            }
        }
        OperationExpressionRefs::Call { arguments } => OperationExpressionRefs::Call {
            arguments: copy_expression_span(target, source_expressions, arguments),
        },
        OperationExpressionRefs::Expression(expression) => OperationExpressionRefs::Expression(
            copy_expression(target, source_expressions, expression),
        ),
        OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

fn remap_transition(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    transition: &TransitionEdge,
) -> TransitionEdge {
    TransitionEdge {
        statement_index: transition.statement_index,
        target: transition.target.clone(),
        continuation: transition.continuation.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments: copy_expression_span(
                target,
                source_expressions,
                transition.expressions.target_arguments,
            ),
            target_value: transition
                .expressions
                .target_value
                .is_valid()
                .then(|| {
                    copy_expression(
                        target,
                        source_expressions,
                        transition.expressions.target_value,
                    )
                })
                .unwrap_or_else(ExpressionHandle::invalid),
            continuation_arguments: copy_expression_span(
                target,
                source_expressions,
                transition.expressions.continuation_arguments,
            ),
            continuation_value: transition
                .expressions
                .continuation_value
                .is_valid()
                .then(|| {
                    copy_expression(
                        target,
                        source_expressions,
                        transition.expressions.continuation_value,
                    )
                })
                .unwrap_or_else(ExpressionHandle::invalid),
            guard: transition
                .expressions
                .guard
                .is_valid()
                .then(|| copy_expression(target, source_expressions, transition.expressions.guard))
                .unwrap_or_else(ExpressionHandle::invalid),
        },
    }
}

fn copy_expression(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    target.expressions.copy_from(source_expressions, expression)
}

fn copy_expression_span(
    target: &mut StateGraph,
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    expressions: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    target
        .expressions
        .copy_expression_handles_from(source_expressions, expressions)
}
