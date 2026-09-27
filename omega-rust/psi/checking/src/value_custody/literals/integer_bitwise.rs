//! Bitwise operations have no anonymous numeric meaning either.
//!
//! [Numeric values](../../../../../../wiki/spec/language/numeric_values.md):
//! "Integer bitwise AND, OR, XOR, and complement preserve their
//! signed/unsigned carrier and are total at its width", and a shift count is
//! proven against that carrier's `value_width`. An operand with no carrier
//! therefore has no defined result, exactly as an anonymous `%` has none, and
//! a destination annotation does not supply one.
//!
//! Without this check such an expression passes checking and then silently
//! omits the machine's unit plan at `structural field store: pure source:
//! scalar expression row` — an author writing `self.mode = 4 | 2;` is told
//! only that the program entry "rejoins 0 Terminal attachment identities".

use diagnostics::Diagnostic;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
};

use super::integer_remainder::is_anonymous_numeric_expression;

pub(crate) fn validate_anonymous_bitwise(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        // These spellings are not overloadable, so a binary node carrying one
        // is the builtin operation and needs no selection evidence.
        let Some(spelling) = bitwise_spelling(binary.operator) else {
            continue;
        };
        if !untyped_operands(program, binary.operator, binary.left, binary.right) {
            continue;
        }
        diagnostics.push(
            Diagnostic::error(bitwise_message(binary.operator, spelling))
                .with_source_span(program.expression_table.source_span(expression)),
        );
    }
}

/// A shift takes its width from the VALUE it shifts, so typing only the count
/// leaves the operation undefined; the symmetric bit operations need either
/// operand typed and land the other.
fn untyped_operands(
    program: &TypedTrees,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    match operator {
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight => {
            is_anonymous_numeric_expression(program, left)
        }
        _ => {
            is_anonymous_numeric_expression(program, left)
                && is_anonymous_numeric_expression(program, right)
        }
    }
}

const fn bitwise_spelling(operator: BinaryOperator) -> Option<&'static str> {
    match operator {
        BinaryOperator::BitwiseAnd => Some("&"),
        BinaryOperator::BitwiseOr => Some("|"),
        BinaryOperator::BitwiseXor => Some("^"),
        BinaryOperator::ShiftLeft => Some("<<"),
        BinaryOperator::ShiftRight => Some(">>"),
        _ => None,
    }
}

fn bitwise_message(operator: BinaryOperator, spelling: &str) -> String {
    if matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    ) {
        return format!(
            "builtin `{spelling}` takes its width from the value it shifts, so that value needs \
             an integer type; type it (for example, `4i32 {spelling} 1`), not just the \
             destination"
        );
    }
    format!(
        "builtin `{spelling}` requires an integer-typed operand; type an operand (for example, \
         `6i32 {spelling} 4`), not just the destination"
    )
}
