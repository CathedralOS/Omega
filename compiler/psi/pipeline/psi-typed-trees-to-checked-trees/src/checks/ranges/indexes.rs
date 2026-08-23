use psi_diagnostics::Diagnostic;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;

mod validation;

use super::facts::RangeFacts;
use validation::check_indexed_access;

pub(super) fn check_expression(
    program: &psi_typed_trees::TypedTrees,
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
        ExpressionNode::Atomic(atomic) => {
            check_expression(program, machine, state, facts, atomic.value, diagnostics)
        }
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
            // A §5b RECAST's operand is an ADDRESS the view starts at, not an
            // element read: its bounds are the recast judgment's FOOTPRINT
            // (offset + size_of(target) <= region, strictly stronger than the
            // element check `offset < region`). Skip the element-index
            // obligation on the direct Indexed operand -- but still walk the
            // INDEX expression itself (a nested read inside the offset
            // computation keeps its own obligations).
            if cast.form.is_recast()
                && let ExpressionNode::Indexed(indexed) =
                    program.expression_table.expression(cast.value)
            {
                check_expression(
                    program,
                    machine,
                    state,
                    facts,
                    indexed.collection,
                    diagnostics,
                );
                check_expression(program, machine, state, facts, indexed.index, diagnostics);
                return;
            }
            check_expression(program, machine, state, facts, cast.value, diagnostics)
        }
        ExpressionNode::Indexed(indexed) => {
            // Operators lane: `check_indexed_access` now sources the `[]` / `[..]`
            // bounds obligation from the spelled boundary operator's `requires`
            // clause (see `validation.rs`), discharged by the bounds proof below.
            check_indexed_access(program, machine, state, facts, indexed, diagnostics);
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
        ExpressionNode::Unary(unary) => {
            check_expression(program, machine, state, facts, unary.operand, diagnostics)
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
