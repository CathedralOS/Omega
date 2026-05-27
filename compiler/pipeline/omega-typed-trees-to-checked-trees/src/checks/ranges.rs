mod expressions;
mod facts;

use expressions::{
    expression_indexable_length, expression_integer_value, expression_name, provable_range_bounds,
};
use facts::{RangeFacts, fixed_array_field_lengths, fixed_array_type_length};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(crate) fn check_indexed_accesses(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut facts = RangeFacts::new(&field_lengths);
            for statement in program.statement_table.statements(state.statement_nodes) {
                check_statement(program, &mut facts, statement, &mut diagnostics);
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_statement(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    statement: &StatementNode,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match statement {
        StatementNode::Assignment(assignment) => {
            check_expression(program, facts, assignment.target, diagnostics);
            check_expression(program, facts, assignment.value, diagnostics);
            if let Some((symbol, name)) = expression_name(program, assignment.target) {
                let next_length = expression_indexable_length(program, facts, assignment.value);
                let next_integer = expression_integer_value(program, facts, assignment.value);
                facts.assign_local(symbol, name, next_length, next_integer);
            }
        }
        StatementNode::Call(call) => {
            for argument in program.statement_table.expression_handles(call.arguments) {
                check_expression(program, facts, *argument, diagnostics);
            }
        }
        StatementNode::Expression(expression) => {
            check_expression(program, facts, *expression, diagnostics);
        }
        StatementNode::LocalData(local) => {
            check_expression(program, facts, local.initial_value, diagnostics);
            if let Some(length) = expression_indexable_length(program, facts, local.initial_value)
                .or_else(|| fixed_array_type_length(program, local.type_reference))
            {
                facts.define_local(local.symbol, local.name.to_string(), Some(length), None);
            } else if let Some(value) =
                expression_integer_value(program, facts, local.initial_value)
            {
                facts.define_local(local.symbol, local.name.to_string(), None, Some(value));
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(guard) = transition.guard {
                check_expression(program, facts, guard, diagnostics);
            }
            check_transition_target(program, facts, transition.target, diagnostics);
            check_transition_target(program, facts, transition.continuation, diagnostics);
        }
    }
}

fn check_transition_target(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !target.is_valid() {
        return;
    }

    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                check_expression(program, facts, *argument, diagnostics);
            }
        }
        TransitionTargetNode::Value(value) => check_expression(program, facts, *value, diagnostics),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn check_expression(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                check_expression(program, facts, *value, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            check_expression(program, facts, binary.left, diagnostics);
            check_expression(program, facts, binary.right, diagnostics);
        }
        ExpressionNode::Call(call) => {
            check_expression(program, facts, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                check_expression(program, facts, *argument, diagnostics);
            }
        }
        ExpressionNode::Cast(cast) => check_expression(program, facts, cast.value, diagnostics),
        ExpressionNode::Indexed(indexed) => {
            if let Some(length) = expression_indexable_length(program, facts, indexed.collection) {
                check_index(program, facts, indexed.index, length, diagnostics);
            }
            check_expression(program, facts, indexed.collection, diagnostics);
            check_expression(program, facts, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            check_expression(program, facts, member.receiver, diagnostics);
        }
        ExpressionNode::Mutable(inner) => check_expression(program, facts, *inner, diagnostics),
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                check_expression(program, facts, range.start, diagnostics);
            }
            if range.end.is_valid() {
                check_expression(program, facts, range.end, diagnostics);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                check_expression(program, facts, field.value, diagnostics);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn check_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    length: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            check_range_index(program, facts, index, range, length, diagnostics)
        }
        _ => {
            let Some(index_value) = expression_integer_value(program, facts, index) else {
                return;
            };
            let valid =
                index_value >= 0 && usize::try_from(index_value).is_ok_and(|index| index < length);
            if !valid {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove index `{}` is within length {}",
                    program.expression_table.display_name(index),
                    length
                )));
            }
        }
    }
}

fn check_range_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    range: &TableRangeExpression,
    length: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((start, end)) = provable_range_bounds(program, facts, range) else {
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range `{}` is within slice length {}",
            program.expression_table.display_name(index),
            length
        )));
        return;
    };

    let invalid = start < 0
        || end.is_some_and(|end| end < 0 || start > end)
        || usize::try_from(start).map_or(true, |start| start > length)
        || end
            .and_then(|end| usize::try_from(end).ok())
            .is_some_and(|end| end > length);

    if invalid {
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range `{}` is within slice length {}",
            program.expression_table.display_name(index),
            length
        )));
    }
}
