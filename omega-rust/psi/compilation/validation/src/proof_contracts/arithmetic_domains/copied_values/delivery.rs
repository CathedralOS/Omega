//! Values that the scalar-association delivery owner can transport.
//!
//! A constructor delivers its supplied fields in authored order. Every other
//! expression is one completed value: evaluating its operands does not bind
//! those operands into the receiving destination. This selects observation
//! work only; the normal scheduler still owns every operand effect.

use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteralField};

/// Preserve the distinction between an empty constructor and a non-constructor:
/// only the latter has its whole expression frame applied by the delivery owner.
pub(super) fn fields(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<&[TableStructLiteralField]> {
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return None;
    };
    Some(program.expression_table.struct_fields(literal.fields))
}

pub(super) fn visit_values(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    visitor: &mut impl FnMut(ExpressionHandle),
) {
    if !expression.is_valid() {
        return;
    }
    visitor(expression);
    if let Some(fields) = fields(program, expression) {
        for field in fields {
            visit_values(program, field.value, visitor);
        }
    }
}
