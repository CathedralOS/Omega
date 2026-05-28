use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;

use super::diagnostics::{
    known_length_range_bound_failure, known_length_range_value_failure,
    unknown_length_range_failure,
};
use super::expressions::{
    expression_indexable_length, expression_integer_value, expression_is_slice,
    provable_range_bounds,
};
use super::facts::RangeFacts;
use super::proofs::{unknown_length_index_is_proven, unknown_length_range_is_proven};

pub(super) fn check_expression(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
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
                check_expression(program, machine, state, facts, *value, diagnostics);
            }
        }
        ExpressionNode::Binary(binary) => {
            check_expression(program, machine, state, facts, binary.left, diagnostics);
            check_expression(program, machine, state, facts, binary.right, diagnostics);
        }
        ExpressionNode::Call(call) => {
            check_expression(program, machine, state, facts, call.receiver, diagnostics);
            for argument in program.expression_table.expression_handles(call.arguments) {
                check_expression(program, machine, state, facts, *argument, diagnostics);
            }
        }
        ExpressionNode::Cast(cast) => {
            check_expression(program, machine, state, facts, cast.value, diagnostics)
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(length) = expression_indexable_length(program, facts, indexed.collection) {
                check_index(
                    program,
                    facts,
                    indexed.collection,
                    indexed.index,
                    length,
                    diagnostics,
                );
            } else if expression_is_slice(program, machine, state, indexed.collection) {
                check_unknown_length_slice_index(
                    program,
                    facts,
                    indexed.collection,
                    indexed.index,
                    diagnostics,
                );
            }
            check_expression(
                program,
                machine,
                state,
                facts,
                indexed.collection,
                diagnostics,
            );
            check_expression(program, machine, state, facts, indexed.index, diagnostics);
        }
        ExpressionNode::Member(member) => {
            check_expression(program, machine, state, facts, member.receiver, diagnostics);
        }
        ExpressionNode::Mutable(inner) => {
            check_expression(program, machine, state, facts, *inner, diagnostics)
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                check_expression(program, machine, state, facts, range.start, diagnostics);
            }
            if range.end.is_valid() {
                check_expression(program, machine, state, facts, range.end, diagnostics);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                check_expression(program, machine, state, facts, field.value, diagnostics);
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
    collection: ExpressionHandle,
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
                let collection_label = program.expression_table.display_name(collection);
                let index_label = program.expression_table.display_name(index);
                if facts.index_is_proven(&collection_label, &index_label) {
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove index `{}` is within length {}",
                    index_label, length
                )));
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

fn check_unknown_length_slice_index(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    collection: ExpressionHandle,
    index: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match program.expression_table.expression(index) {
        ExpressionNode::Range(range) => {
            if unknown_length_range_is_proven(program, facts, collection, range) {
                return;
            }
            let failure = unknown_length_range_failure(program, facts, collection, range);
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove subslice range {} `{}` is within unknown slice length",
                failure.label(),
                program.expression_table.display_name(index)
            )));
        }
        _ => {
            let index_label = program.expression_table.display_name(index);
            if unknown_length_index_is_proven(program, facts, collection, index) {
                return;
            }
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove index `{}` is within unknown slice length",
                index_label
            )));
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
        let failure = known_length_range_value_failure(program, facts, range);
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range {} `{}` is within slice length {}",
            failure.label(),
            program.expression_table.display_name(index),
            length
        )));
        return;
    };

    if let Some(failure) = known_length_range_bound_failure(start, end, length) {
        diagnostics.push(Diagnostic::error(format!(
            "cannot prove subslice range {} `{}` is within slice length {}",
            failure.label(),
            program.expression_table.display_name(index),
            length
        )));
    }
}
