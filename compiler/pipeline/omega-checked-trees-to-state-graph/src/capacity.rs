use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTableCapacity};
use omega_checked_trees::machine::Machine;
use omega_core::arena::HandleSpan;
use omega_state_graph::StateGraph;

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
