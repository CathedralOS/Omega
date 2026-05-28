use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTableCapacity};
use omega_checked_trees::machine::Machine;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_state_graph::{
    ContainedGraph, InvariantFact, MachineGraph, MachineOwnedDataGraph, Operation,
    OperationExpressionRefs, PlannedTransitionTarget, ProofFactKind, ProofObligationFact,
    ProofObligationOwner, StateBorrowAccessKind, StateBorrowActivation, StateBorrowArgumentAccess,
    StateBorrowCall, StateBorrowEventSource, StateBorrowLoan, StateBorrowRootKind,
    StateBorrowSummary, StateBorrowWeakening, StateBorrowWeakeningReason, StateBorrowWritableRoot,
    StateContractCall, StateContractExit, StateContractFactKind, StateContractFactRef,
    StateContractSummary, StateGraph, StateKey, StateNode, StateParameterNode, TransitionEdge,
    TransitionExpressionRefs,
};
use std::sync::Arc;

use crate::segments::{segment_has_unconditional_transition, split_state_segments};
use crate::transitions::plan_transition;

pub(crate) fn build_state_graph_with_workers(
    program: Arc<CheckedTrees>,
    workers: WorkerPoolHandle,
) -> Result<StateGraph, Diagnostic> {
    if program.machines().is_empty() {
        return Ok(StateGraph::with_capacity(
            ExpressionTableCapacity::default(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
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
    borrow_writable_roots: usize,
    borrow_access_segments: usize,
    borrow_argument_accesses: usize,
    borrow_calls: usize,
    borrow_loans: usize,
    borrow_activations: usize,
    borrow_weakenings: usize,
    operations: usize,
    transitions: usize,
}

impl StateGraphCapacity {
    fn for_program(program: &CheckedTrees) -> Self {
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
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_access_segments: program.facts.borrow.access_segments.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            borrow_loans: program.facts.borrow.loans.len(),
            borrow_activations: program.facts.flow.borrow_activations.len(),
            borrow_weakenings: program.facts.flow.borrow_weakenings.len(),
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
            borrow_writable_roots: program.facts.borrow.writable_roots.len(),
            borrow_access_segments: program.facts.borrow.access_segments.len(),
            borrow_argument_accesses: program.facts.borrow.argument_accesses.len(),
            borrow_calls: program.facts.borrow.calls.len(),
            borrow_loans: program.facts.borrow.loans.len(),
            borrow_activations: program.facts.flow.borrow_activations.len(),
            borrow_weakenings: program.facts.flow.borrow_weakenings.len(),
            operations: statement_capacity,
            transitions: statement_capacity,
        }
    }

    fn into_state_graph(self) -> StateGraph {
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
            self.borrow_writable_roots,
            self.borrow_access_segments,
            self.borrow_argument_accesses,
            self.borrow_calls,
            self.borrow_loans,
            self.borrow_activations,
            self.borrow_weakenings,
            self.operations,
            self.transitions,
        )
    }
}

fn machine_expression_capacity(
    program: &CheckedTrees,
    machine: &Machine,
) -> ExpressionTableCapacity {
    program
        .machine_states(machine)
        .iter()
        .flat_map(|state| program.statement_table.statements(state.statement_nodes))
        .fold(
            ExpressionTableCapacity::default(),
            |mut capacity, statement| {
                capacity.saturating_add_assign(statement_expression_capacity(program, statement));
                capacity
            },
        )
}

fn statement_expression_capacity(
    program: &CheckedTrees,
    statement: &omega_checked_trees::statement::StatementNode,
) -> ExpressionTableCapacity {
    match statement {
        omega_checked_trees::statement::StatementNode::Assignment(assignment) => {
            let mut capacity = copied_expression_capacity(program, assignment.target);
            capacity.saturating_add_assign(copied_expression_capacity(program, assignment.value));
            capacity
        }
        omega_checked_trees::statement::StatementNode::Call(call) => {
            expression_span_capacity(program, call.arguments)
        }
        omega_checked_trees::statement::StatementNode::Expression(expression) => {
            copied_expression_capacity(program, *expression)
        }
        omega_checked_trees::statement::StatementNode::Transition(transition) => {
            let mut capacity = transition_guard_expression_capacity(program, transition.guard);
            capacity.saturating_add_assign(transition_target_expression_capacity(
                program,
                transition.target,
            ));
            capacity.saturating_add_assign(transition_target_expression_capacity(
                program,
                transition.continuation,
            ));
            capacity
        }
        omega_checked_trees::statement::StatementNode::LocalData(_) => {
            ExpressionTableCapacity::default()
        }
    }
}

fn expression_span_capacity(
    program: &CheckedTrees,
    expressions: HandleSpan<ExpressionHandle>,
) -> ExpressionTableCapacity {
    let handles = program.statement_table.expression_handles(expressions);
    let mut capacity = ExpressionTableCapacity {
        expression_handles: handles.len(),
        ..ExpressionTableCapacity::default()
    };
    for expression in handles {
        capacity.saturating_add_assign(copied_expression_capacity(program, *expression));
    }
    capacity
}

fn transition_guard_expression_capacity(
    program: &CheckedTrees,
    guard: omega_checked_trees::statement::TransitionGuardNode,
) -> ExpressionTableCapacity {
    match guard {
        omega_checked_trees::statement::TransitionGuardNode::Always => {
            ExpressionTableCapacity::default()
        }
        omega_checked_trees::statement::TransitionGuardNode::When(expression) => {
            copied_expression_capacity(program, expression)
        }
    }
}

fn transition_target_expression_capacity(
    program: &CheckedTrees,
    target: omega_checked_trees::statement::TransitionTargetHandle,
) -> ExpressionTableCapacity {
    if !target.is_valid() {
        return ExpressionTableCapacity::default();
    }

    match program.statement_table.transition_target(target) {
        omega_checked_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            expression_span_capacity(program, *arguments)
        }
        omega_checked_trees::statement::TransitionTargetNode::Value(expression) => {
            copied_expression_capacity(program, *expression)
        }
        omega_checked_trees::statement::TransitionTargetNode::SelfTarget
        | omega_checked_trees::statement::TransitionTargetNode::Terminal => {
            ExpressionTableCapacity::default()
        }
    }
}

fn copied_expression_capacity(
    program: &CheckedTrees,
    expression: ExpressionHandle,
) -> ExpressionTableCapacity {
    if !expression.is_valid() {
        return ExpressionTableCapacity::default();
    }

    let mut capacity = ExpressionTableCapacity {
        expressions: 1,
        ..ExpressionTableCapacity::default()
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            capacity.saturating_add_assign(expression_table_span_capacity(program, *values));
        }
        ExpressionNode::Binary(binary) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, binary.left));
            capacity.saturating_add_assign(copied_expression_capacity(program, binary.right));
        }
        ExpressionNode::Cast(cast) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, cast.value));
            capacity.name_path_members = capacity
                .name_path_members
                .saturating_add(span_count(cast.target_type));
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                capacity.saturating_add_assign(copied_expression_capacity(program, call.receiver));
            }
            capacity.saturating_add_assign(expression_table_span_capacity(program, call.arguments));
        }
        ExpressionNode::Indexed(indexed) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, indexed.collection));
            capacity.saturating_add_assign(copied_expression_capacity(program, indexed.index));
        }
        ExpressionNode::Range(range) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, range.start));
            capacity.saturating_add_assign(copied_expression_capacity(program, range.end));
        }
        ExpressionNode::Member(member) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, member.receiver));
        }
        ExpressionNode::Mutable(inner) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, *inner));
        }
        ExpressionNode::Name(path) => {
            capacity.name_path_members = capacity
                .name_path_members
                .saturating_add(span_count(path.members));
            capacity.name_path_member_symbols = capacity
                .name_path_member_symbols
                .saturating_add(span_count(path.member_symbols));
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            let fields = program
                .expression_table
                .struct_fields(struct_literal.fields);
            capacity.struct_fields = capacity.struct_fields.saturating_add(fields.len());
            for field in fields {
                capacity.saturating_add_assign(copied_expression_capacity(program, field.value));
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
    capacity
}

fn expression_table_span_capacity(
    program: &CheckedTrees,
    expressions: HandleSpan<ExpressionHandle>,
) -> ExpressionTableCapacity {
    let handles = program.expression_table.expression_handles(expressions);
    let mut capacity = ExpressionTableCapacity {
        expression_handles: handles.len(),
        ..ExpressionTableCapacity::default()
    };
    for expression in handles {
        capacity.saturating_add_assign(copied_expression_capacity(program, *expression));
    }
    capacity
}

fn span_count<T>(span: HandleSpan<T>) -> usize {
    usize::try_from(span.count()).expect("handle span count overflow")
}

fn machine_statement_count(program: &CheckedTrees, machine: &Machine) -> usize {
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
    } = source;

    let states = append_remapped_states(
        target,
        &expressions,
        states.into_span_items(machine_graph.states),
        &state_parameters,
        &contract_fact_refs,
        &contract_calls,
        &contract_exits,
        &borrow_writable_roots,
        &borrow_access_segments,
        &borrow_argument_accesses,
        &borrow_calls,
        &borrow_loans,
        &borrow_activations,
        &borrow_weakenings,
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
        attached_data: machine_graph.attached_data,
        direct_effects: machine_graph.direct_effects,
        reached_effects: machine_graph.reached_effects,
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
    source_contract_fact_refs: &Arena<StateContractFactRef>,
    source_contract_calls: &Arena<StateContractCall>,
    source_contract_exits: &Arena<StateContractExit>,
    source_borrow_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_borrow_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_borrow_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_borrow_calls: &Arena<StateBorrowCall>,
    source_borrow_loans: &Arena<StateBorrowLoan>,
    source_borrow_activations: &Arena<StateBorrowActivation>,
    source_borrow_weakenings: &Arena<StateBorrowWeakening>,
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
        let contracts = remap_state_contract_summary(
            target,
            source_contract_fact_refs,
            source_contract_calls,
            source_contract_exits,
            &state.contracts,
        );
        let borrow = remap_state_borrow_summary(
            target,
            source_borrow_writable_roots,
            source_borrow_access_segments,
            source_borrow_argument_accesses,
            source_borrow_calls,
            source_borrow_loans,
            source_borrow_activations,
            source_borrow_weakenings,
            &state.borrow,
        );
        target.states.append_to_span(
            &mut remapped_states,
            StateNode {
                key: state.key,
                name: state.name,
                index: state.index,
                direct_effects: state.direct_effects,
                reached_effects: state.reached_effects,
                parameters,
                contracts,
                borrow,
                operations,
                transitions,
            },
        );
    }

    remapped_states
}

fn remap_state_contract_summary(
    target: &mut StateGraph,
    source_fact_refs: &Arena<StateContractFactRef>,
    source_calls: &Arena<StateContractCall>,
    source_exits: &Arena<StateContractExit>,
    contracts: &StateContractSummary,
) -> StateContractSummary {
    let calls =
        append_remapped_contract_calls(target, source_fact_refs, source_calls, contracts.calls);
    let exits =
        append_remapped_contract_exits(target, source_fact_refs, source_exits, contracts.exits);

    StateContractSummary { calls, exits }
}

fn append_remapped_contract_calls(
    target: &mut StateGraph,
    source_fact_refs: &Arena<StateContractFactRef>,
    source_calls: &Arena<StateContractCall>,
    calls: HandleSpan<StateContractCall>,
) -> HandleSpan<StateContractCall> {
    let mut remapped_calls = HandleSpan::empty();

    for call in source_calls.span_or_empty(calls) {
        let requires = target.contract_fact_refs.insert_many(
            source_fact_refs
                .span_or_empty(call.requires)
                .iter()
                .cloned(),
        );
        let ensures = target
            .contract_fact_refs
            .insert_many(source_fact_refs.span_or_empty(call.ensures).iter().cloned());

        target.contract_calls.append_to_span(
            &mut remapped_calls,
            StateContractCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine_symbol: call.target_machine_symbol,
                target_state_symbol: call.target_state_symbol,
                requires,
                ensures,
            },
        );
    }

    remapped_calls
}

fn append_remapped_contract_exits(
    target: &mut StateGraph,
    source_fact_refs: &Arena<StateContractFactRef>,
    source_exits: &Arena<StateContractExit>,
    exits: HandleSpan<StateContractExit>,
) -> HandleSpan<StateContractExit> {
    let mut remapped_exits = HandleSpan::empty();

    for exit in source_exits.span_or_empty(exits) {
        let ensures = target
            .contract_fact_refs
            .insert_many(source_fact_refs.span_or_empty(exit.ensures).iter().cloned());

        target.contract_exits.append_to_span(
            &mut remapped_exits,
            StateContractExit {
                statement_index: exit.statement_index,
                ensures,
            },
        );
    }

    remapped_exits
}

fn remap_state_borrow_summary(
    target: &mut StateGraph,
    source_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_calls: &Arena<StateBorrowCall>,
    source_loans: &Arena<StateBorrowLoan>,
    source_activations: &Arena<StateBorrowActivation>,
    source_weakenings: &Arena<StateBorrowWeakening>,
    borrow: &StateBorrowSummary,
) -> StateBorrowSummary {
    let writable_roots = target.borrow_writable_roots.insert_many(
        source_writable_roots
            .span_or_empty(borrow.writable_roots)
            .iter()
            .cloned(),
    );

    let calls = append_remapped_borrow_calls(
        target,
        source_access_segments,
        source_argument_accesses,
        source_calls,
        borrow.calls,
    );
    let active_loans = append_remapped_borrow_loans(
        target,
        source_access_segments,
        source_loans,
        borrow.active_loans,
    );
    let activations = append_remapped_borrow_activations(
        target,
        source_access_segments,
        source_loans,
        source_activations,
        borrow.activations,
    );
    let weakenings = append_remapped_borrow_weakenings(
        target,
        source_access_segments,
        source_loans,
        source_weakenings,
        borrow.weakenings,
    );

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: borrow.mutable_parameter_count,
        calls,
        active_loans,
        activations,
        weakenings,
    }
}

fn append_remapped_borrow_calls(
    target: &mut StateGraph,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
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
                .map(|access| StateBorrowArgumentAccess {
                    root_symbol: access.root_symbol,
                    segments: target.borrow_access_segments.insert_many(
                        source_access_segments
                            .span_or_empty(access.segments)
                            .iter()
                            .copied(),
                    ),
                    kind: access.kind.clone(),
                }),
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

fn append_remapped_borrow_loans(
    target: &mut StateGraph,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    loans: HandleSpan<StateBorrowLoan>,
) -> HandleSpan<StateBorrowLoan> {
    let mut remapped_loans = HandleSpan::empty();

    for loan in source_loans.span_or_empty(loans) {
        target.borrow_loans.append_to_span(
            &mut remapped_loans,
            StateBorrowLoan {
                statement_index: loan.statement_index,
                last_use_statement_index: loan.last_use_statement_index,
                owner_symbol: loan.owner_symbol,
                root_symbol: loan.root_symbol,
                segments: target.borrow_access_segments.insert_many(
                    source_access_segments
                        .span_or_empty(loan.segments)
                        .iter()
                        .copied(),
                ),
            },
        );
    }

    remapped_loans
}

fn append_remapped_borrow_activations(
    target: &mut StateGraph,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    source_activations: &Arena<StateBorrowActivation>,
    activations: HandleSpan<StateBorrowActivation>,
) -> HandleSpan<StateBorrowActivation> {
    let mut remapped = HandleSpan::empty();
    let mut loan_map: Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)> = Vec::new();

    for activation in source_activations.span_or_empty(activations) {
        let loan = remapped_loan_handle(
            target,
            source_access_segments,
            source_loans,
            activation.loan,
            &mut loan_map,
        );
        target.borrow_activations.append_to_span(
            &mut remapped,
            StateBorrowActivation {
                source: activation.source.clone(),
                loan,
            },
        );
    }

    remapped
}

fn append_remapped_borrow_weakenings(
    target: &mut StateGraph,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    source_weakenings: &Arena<StateBorrowWeakening>,
    weakenings: HandleSpan<StateBorrowWeakening>,
) -> HandleSpan<StateBorrowWeakening> {
    let mut remapped = HandleSpan::empty();
    let mut loan_map: Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)> = Vec::new();

    for weakening in source_weakenings.span_or_empty(weakenings) {
        let loan = remapped_loan_handle(
            target,
            source_access_segments,
            source_loans,
            weakening.loan,
            &mut loan_map,
        );
        target.borrow_weakenings.append_to_span(
            &mut remapped,
            StateBorrowWeakening {
                source: weakening.source.clone(),
                loan,
                reason: weakening.reason,
            },
        );
    }

    remapped
}

fn remapped_loan_handle(
    target: &mut StateGraph,
    source_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_loans: &Arena<StateBorrowLoan>,
    source_loan: Handle<StateBorrowLoan>,
    loan_map: &mut Vec<(Handle<StateBorrowLoan>, Handle<StateBorrowLoan>)>,
) -> Handle<StateBorrowLoan> {
    if let Some((_, mapped)) = loan_map
        .iter()
        .find(|(candidate, _)| *candidate == source_loan)
    {
        return *mapped;
    }

    let loan = source_loans.get(source_loan);
    let mapped = target.borrow_loans.append(StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: target.borrow_access_segments.insert_many(
            source_access_segments
                .span_or_empty(loan.segments)
                .iter()
                .copied(),
        ),
    });
    loan_map.push((source_loan, mapped));
    mapped
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
    program: &CheckedTrees,
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

    let (direct_effects, reached_effects) = machine_effect_bits(program, machine_symbol);

    Ok(MachineGraph {
        symbol: machine_symbol,
        name: machine.name.clone(),
        attached_data: machine.attached_data.clone(),
        direct_effects,
        reached_effects,
        contains: machine_contains(state_graph, program, machine),
        owned_data: machine_owned_data(state_graph, program, machine),
        states,
    })
}

fn machine_owned_data(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
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

fn machine_effect_bits(
    program: &CheckedTrees,
    machine_symbol: omega_core::symbols::SymbolHandle,
) -> (omega_effects::EffectBits, omega_effects::EffectBits) {
    program
        .facts
        .effects
        .machines()
        .iter()
        .find(|effects| effects.symbol == machine_symbol)
        .map(|effects| (effects.direct.bits(), effects.transitive.bits()))
        .unwrap_or_default()
}

fn state_effect_bits(
    program: &CheckedTrees,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> (omega_effects::EffectBits, omega_effects::EffectBits) {
    program
        .facts
        .effects
        .machines()
        .iter()
        .flat_map(|machine| program.facts.effects.states.span_or_empty(machine.states))
        .find(|effects| effects.symbol == state_symbol)
        .map(|effects| (effects.direct.bits(), effects.transitive.bits()))
        .unwrap_or_default()
}

fn estimated_machine_segment_capacity(program: &CheckedTrees, machine: &Machine) -> usize {
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
    program: &CheckedTrees,
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
        .find(|data_definition| Some(&data_definition.name) == machine.attached_data.as_ref())
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
            .find(|candidate| candidate.attached_data.as_ref() == Some(&field_type_name))
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
                type_name: field_type_name,
            },
        );
    }

    contains
}

fn type_reference_name_handle(
    program: &CheckedTrees,
    type_reference: omega_checked_trees::types::TypeReferenceHandle,
) -> omega_checked_trees::name::Identifier {
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
            omega_checked_trees::name::Identifier::default()
        }
    }
}

fn append_machine_states(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segments: &[crate::segments::StateSegment],
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
) -> Result<HandleSpan<StateNode>, Diagnostic> {
    let mut states = HandleSpan::empty();

    for (index, segment) in segments.iter().enumerate() {
        let (direct_effects, reached_effects) = state_effect_bits(program, segment.key.state);
        let transitions = append_segment_transitions(
            state_graph,
            program,
            segment,
            segments,
            segment_transitions,
        )?;
        let contracts = state_contract_summary(state_graph, program, segment, segment_transitions);
        let borrow = state_borrow_summary(state_graph, program, segment.key);
        state_graph.states.append_to_span(
            &mut states,
            StateNode {
                key: segment.key,
                name: segment.name.clone(),
                index,
                direct_effects,
                reached_effects,
                parameters: segment.parameters,
                contracts,
                borrow,
                operations: segment.operations,
                transitions,
            },
        );
    }

    Ok(states)
}

fn state_contract_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segment: &crate::segments::StateSegment,
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
) -> StateContractSummary {
    let mut calls = HandleSpan::empty();
    for (_, call) in program.facts.proof.contract_calls.iter() {
        if call.caller_machine_symbol != segment.key.machine
            || call.caller_state_symbol != segment.key.state
            || !segment_contains_statement_index(
                state_graph,
                segment,
                segment_transitions,
                call.statement_index,
            )
        {
            continue;
        }

        let requires = append_state_contract_fact_refs(state_graph, program, call.requires);
        let ensures = append_state_contract_fact_refs(state_graph, program, call.ensures);
        state_graph.contract_calls.append_to_span(
            &mut calls,
            StateContractCall {
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
                target_machine_symbol: call.target_machine_symbol,
                target_state_symbol: call.target_state_symbol,
                requires,
                ensures,
            },
        );
    }

    let mut exits = HandleSpan::empty();
    for (_, exit) in program.facts.proof.contract_exits.iter() {
        if exit.machine_symbol != segment.key.machine
            || exit.state_symbol != segment.key.state
            || !segment_contains_statement_index(
                state_graph,
                segment,
                segment_transitions,
                exit.statement_index,
            )
        {
            continue;
        }

        let ensures = append_state_contract_fact_refs(state_graph, program, exit.ensures);
        state_graph.contract_exits.append_to_span(
            &mut exits,
            StateContractExit {
                statement_index: exit.statement_index,
                ensures,
            },
        );
    }

    StateContractSummary { calls, exits }
}

fn segment_contains_statement_index(
    state_graph: &StateGraph,
    segment: &crate::segments::StateSegment,
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
    statement_index: usize,
) -> bool {
    state_graph
        .operations
        .span_or_empty(segment.operations)
        .iter()
        .any(|operation| operation.statement_index == statement_index)
        || segment_transitions
            .span_or_empty(segment.transitions)
            .iter()
            .any(|transition| match transition {
                crate::segments::SegmentTransition::Tree {
                    statement_index: transition_statement_index,
                    ..
                }
                | crate::segments::SegmentTransition::ReturnExpression {
                    statement_index: transition_statement_index,
                    ..
                }
                | crate::segments::SegmentTransition::BranchCall {
                    statement_index: transition_statement_index,
                    ..
                } => *transition_statement_index == statement_index,
            })
}

fn append_state_contract_fact_refs(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    refs: HandleSpan<omega_checked_trees::ContractProofFactRef>,
) -> HandleSpan<StateContractFactRef> {
    let mut fact_refs = HandleSpan::empty();

    for reference in program.facts.proof.contract_fact_refs.span_or_empty(refs) {
        let contract_fact = program.facts.proof.contract_facts.get(reference.fact);
        state_graph.contract_fact_refs.append_to_span(
            &mut fact_refs,
            StateContractFactRef {
                kind: match contract_fact.kind {
                    omega_checked_trees::ContractProofFactKind::Requires => {
                        StateContractFactKind::Requires
                    }
                    omega_checked_trees::ContractProofFactKind::Ensures => {
                        StateContractFactKind::Ensures
                    }
                    omega_checked_trees::ContractProofFactKind::Boundary => {
                        StateContractFactKind::Boundary
                    }
                },
                fact: contract_fact.fact,
            },
        );
    }

    fact_refs
}

fn state_borrow_summary(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
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
                    segments: state_graph.borrow_access_segments.insert_many(
                        program
                            .facts
                            .borrow
                            .access_segments
                            .span_or_empty(access.segments)
                            .iter()
                            .copied(),
                    ),
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

    let mut active_loan_start: Option<Handle<StateBorrowLoan>> = None;
    let mut active_loan_count = 0usize;
    let mut activations = HandleSpan::empty();
    let mut weakenings = HandleSpan::empty();
    let mut loan_map: Vec<(
        Handle<omega_checked_trees::BorrowLoanFact>,
        Handle<StateBorrowLoan>,
    )> = Vec::new();

    if let Some(flow_state) = program
        .facts
        .flow
        .states
        .iter()
        .find(|(_, state)| state.machine_symbol == key.machine && state.state_symbol == key.state)
        .map(|(_, state)| state)
    {
        for source_loan in program
            .facts
            .flow
            .borrow_loan_constraints(flow_state.entry_constraints)
        {
            let loan = ensure_state_borrow_loan(state_graph, program, source_loan, &mut loan_map);
            active_loan_start.get_or_insert(loan);
            active_loan_count += 1;
        }

        for activation in program
            .facts
            .flow
            .borrow_activations
            .span_or_empty(flow_state.borrow_activations)
        {
            let loan =
                ensure_state_borrow_loan(state_graph, program, activation.loan, &mut loan_map);
            state_graph.borrow_activations.append_to_span(
                &mut activations,
                StateBorrowActivation {
                    source: remap_flow_borrow_event_source(activation.source),
                    loan,
                },
            );
        }

        for weakening in program
            .facts
            .flow
            .borrow_weakenings
            .span_or_empty(flow_state.borrow_weakenings)
        {
            let loan =
                ensure_state_borrow_loan(state_graph, program, weakening.loan, &mut loan_map);
            state_graph.borrow_weakenings.append_to_span(
                &mut weakenings,
                StateBorrowWeakening {
                    source: remap_flow_borrow_event_source(weakening.source),
                    loan,
                    reason: remap_flow_borrow_weakening_reason(weakening.reason),
                },
            );
        }
    }

    let active_loans = active_loan_start
        .map(|start| {
            HandleSpan::from_parts(
                start,
                active_loan_count
                    .try_into()
                    .expect("active loan count should fit in u32"),
            )
        })
        .unwrap_or_else(HandleSpan::empty);

    StateBorrowSummary {
        writable_roots,
        mutable_parameter_count: state_borrow.mutable_parameter_count,
        calls,
        active_loans,
        activations,
        weakenings,
    }
}

fn ensure_state_borrow_loan(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    source_loan: Handle<omega_checked_trees::BorrowLoanFact>,
    loan_map: &mut Vec<(
        Handle<omega_checked_trees::BorrowLoanFact>,
        Handle<StateBorrowLoan>,
    )>,
) -> Handle<StateBorrowLoan> {
    if let Some((_, mapped)) = loan_map
        .iter()
        .find(|(candidate, _)| *candidate == source_loan)
    {
        return *mapped;
    }

    let loan = program.facts.borrow.loans.get(source_loan);
    let mapped = state_graph.borrow_loans.append(StateBorrowLoan {
        statement_index: loan.statement_index,
        last_use_statement_index: loan.last_use_statement_index,
        owner_symbol: loan.owner_symbol,
        root_symbol: loan.root_symbol,
        segments: state_graph.borrow_access_segments.insert_many(
            program
                .facts
                .borrow
                .access_segments
                .span_or_empty(loan.segments)
                .iter()
                .copied(),
        ),
    });
    loan_map.push((source_loan, mapped));
    mapped
}

fn remap_flow_borrow_event_source(
    source: omega_checked_trees::FlowInvalidationSource,
) -> StateBorrowEventSource {
    match source {
        omega_checked_trees::FlowInvalidationSource::Statement { statement_index } => {
            StateBorrowEventSource::Statement { statement_index }
        }
        omega_checked_trees::FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => StateBorrowEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        },
    }
}

fn remap_flow_borrow_weakening_reason(
    reason: omega_checked_trees::FlowBorrowWeakeningReason,
) -> StateBorrowWeakeningReason {
    match reason {
        omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired => {
            StateBorrowWeakeningReason::LastUseExpired
        }
        omega_checked_trees::FlowBorrowWeakeningReason::StateExit => {
            StateBorrowWeakeningReason::StateExit
        }
        omega_checked_trees::FlowBorrowWeakeningReason::LocalReassigned => {
            StateBorrowWeakeningReason::LocalReassigned
        }
    }
}

fn append_segment_transitions(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
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
