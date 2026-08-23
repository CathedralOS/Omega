use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTableCapacity};
use psi_checked_trees::machine::Machine;

pub(super) fn machine_expression_capacity(
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
    statement: &psi_checked_trees::statement::StatementNode,
) -> ExpressionTableCapacity {
    match statement {
        psi_checked_trees::statement::StatementNode::AssemblyFact(_) => {
            ExpressionTableCapacity::default()
        }
        psi_checked_trees::statement::StatementNode::Assignment(assignment) => {
            let mut capacity = copied_expression_capacity(program, assignment.target);
            capacity.saturating_add_assign(copied_expression_capacity(program, assignment.value));
            capacity
        }
        psi_checked_trees::statement::StatementNode::Call(call) => {
            expression_span_capacity(program, call.arguments)
        }
        psi_checked_trees::statement::StatementNode::Expression(expression) => {
            copied_expression_capacity(program, *expression)
        }
        psi_checked_trees::statement::StatementNode::Transition(transition) => {
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
        psi_checked_trees::statement::StatementNode::LocalData(_) => {
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
    guard: psi_checked_trees::statement::TransitionGuardNode,
) -> ExpressionTableCapacity {
    match guard {
        psi_checked_trees::statement::TransitionGuardNode::Always => {
            ExpressionTableCapacity::default()
        }
        psi_checked_trees::statement::TransitionGuardNode::When(expression) => {
            copied_expression_capacity(program, expression)
        }
    }
}

fn transition_target_expression_capacity(
    program: &CheckedTrees,
    target: psi_checked_trees::statement::TransitionTargetHandle,
) -> ExpressionTableCapacity {
    if !target.is_valid() {
        return ExpressionTableCapacity::default();
    }

    match program.statement_table.transition_target(target) {
        psi_checked_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            expression_span_capacity(program, *arguments)
        }
        psi_checked_trees::statement::TransitionTargetNode::Value(expression) => {
            copied_expression_capacity(program, *expression)
        }
        psi_checked_trees::statement::TransitionTargetNode::SelfTarget
        | psi_checked_trees::statement::TransitionTargetNode::Terminal => {
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
        ExpressionNode::Atomic(atomic) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, atomic.value));
        }
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
                .saturating_add(span_count(cast.target_label));
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
        ExpressionNode::Borrow(inner) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, inner.target));
        }
        ExpressionNode::Unary(unary) => {
            capacity.saturating_add_assign(copied_expression_capacity(program, unary.operand));
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
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
