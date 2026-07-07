//! D14 literal-width gate: which positions may hold a u64-magnitude literal.
//!
//! Literals are anonymous payloads with no parse-time ceiling (see
//! `omega_core::literals::IntegerLiteral`), so a u64-magnitude spelling like
//! `18446744073709551615` PARSES. Every consumer reads literals through the
//! i64 value window (`value_i64()`) and defers/degrades on `None`; the backend
//! write path additionally materializes full 8-byte patterns via `bits_u64()`.
//! That is only sound because THIS gate guarantees an oversize literal reaches
//! exactly the positions that handle it:
//!
//! - **Accepted (fire C):** the direct RHS of an assignment whose target's
//!   declared primitive is u64-classed (`u64`/`usize`/`addr`) -- an 8-byte
//!   slot, so the two's-complement bit pattern the write path emits is the
//!   value. (The interval side already agrees: an oversize-positive literal's
//!   honest over-approximation `[i64::MAX, +inf)` fits only u64-classed
//!   target ranges. The gate must stay PRECISE regardless -- a `u32 in
//!   Wrapping` target bypasses the interval store-check entirely, and only
//!   this gate stands between such a slot and a silent truncation.)
//! - **Everything else** (arithmetic operands, guards, call/transition
//!   arguments, constructions, narrower or signed targets): one clear error.
//!
//! Growing acceptance to a new position = extend `u64_blessed_literals` AND
//! the consumer that materializes it, in the SAME change.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::StatementNode;
use omega_typed_trees::types::PrimitiveType;

pub(crate) fn validate_literal_widths(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let blessed = u64_blessed_literals(program);
    for (handle, node) in program.expression_table.expression_entries() {
        if let ExpressionNode::Integer(literal) = node
            && literal.value_i64().is_none()
            && !blessed.contains(&handle)
        {
            diagnostics.push(Diagnostic::error(format!(
                "integer literal `{literal}` exceeds the i64 range; only a direct \
                 assignment into a `u64`/`usize`/`addr` place accepts a u64-magnitude \
                 literal so far (bind it to such a place first, or wait for this \
                 position's typed lowering -- TASKS_TIME.md D14)"
            )));
        }
    }
}

/// The u64-magnitude literals sitting in an ACCEPTED position: the direct RHS
/// of an assignment to a u64-classed place.
fn u64_blessed_literals(program: &TypedTrees) -> Vec<ExpressionHandle> {
    // A handful of entries at most -- a Vec beats hashing arena handles.
    let mut blessed = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::Assignment(assignment) = statement else {
                    continue;
                };
                let ExpressionNode::Integer(literal) =
                    program.expression_table.expression(assignment.value)
                else {
                    continue;
                };
                if literal.value_i64().is_some() || literal.value_u64().is_none() {
                    continue;
                }
                let Some(declared) = crate::places::declared_place_type_raw(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                ) else {
                    continue;
                };
                let Some(unwrapped) = crate::places::unwrapped_type_reference(program, declared)
                else {
                    continue;
                };
                let Some(primitive) = program.primitive_type_reference(unwrapped) else {
                    continue;
                };
                if matches!(
                    primitive,
                    PrimitiveType::U64 | PrimitiveType::Usize | PrimitiveType::Addr
                ) {
                    blessed.push(assignment.value);
                }
            }
        }
    }
    blessed
}
