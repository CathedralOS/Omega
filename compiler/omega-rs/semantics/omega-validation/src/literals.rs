//! D14 fire-B gate: oversize integer literals are rejected HERE, loudly.
//!
//! Literals are anonymous payloads with no parse-time ceiling (see
//! `omega_core::literals::IntegerLiteral`), so a u64-magnitude spelling like
//! `18446744073709551615` now PARSES. Every consumer reads literals through
//! the i64 value window (`value_i64()`) and defers/degrades on `None` -- which
//! is only sound because THIS pass guarantees such a literal never survives
//! validation. When a position learns to accept u64-magnitude literals (a
//! typed `value_for(target)` lowering -- the next D14 rung, TASKS_TIME.md),
//! it must be excluded here in the same change, never before.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionNode;

pub(crate) fn validate_literal_widths(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for node in program.expression_table.expression_nodes() {
        if let ExpressionNode::Integer(literal) = node
            && literal.value_i64().is_none()
        {
            diagnostics.push(Diagnostic::error(format!(
                "integer literal `{literal}` exceeds the i64 range; no position accepts \
                 u64-magnitude literals yet (the literal itself now parses -- typed \
                 u64 acceptance is the next rung of TASKS_TIME.md D14)"
            )));
        }
    }
}
