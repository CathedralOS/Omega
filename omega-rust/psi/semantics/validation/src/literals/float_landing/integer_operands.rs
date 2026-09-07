//! Integer spelling does not authorize a typed integer-to-float conversion.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

pub(in crate::literals) fn validate_integer_tree_destination(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) {
        return;
    }
    let Some(has_typed_operand) = integer_literal_tree(program, expression) else {
        // Calls, places, casts and authored operators retain their ordinary
        // type/selection checks. This check evaluates no user-defined meaning.
        return;
    };
    let message = if has_typed_operand {
        format!(
            "typed integer arithmetic cannot implicitly land in `{}`; use an explicit numeric conversion",
            destination.name()
        )
    } else {
        let mut builtin =
            |expression| crate::literals::has_anonymous_operator_meaning(program, expression);
        if crate::literals::anonymous_numeric_value(program, expression, &mut builtin).is_some() {
            return;
        }
        format!(
            "anonymous integer arithmetic has no exact value for `{}` landing (division by zero is invalid)",
            destination.name()
        )
    };
    diagnostics.push(
        Diagnostic::error(message)
            .with_source_span(program.expression_table.source_span(expression)),
    );
}

fn integer_literal_tree(program: &TypedTrees, expression: ExpressionHandle) -> Option<bool> {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    let mut has_typed_operand = false;
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => has_typed_operand |= literal.landing().is_some(),
            ExpressionNode::Binary(binary)
                if crate::literals::has_anonymous_operator_meaning(program, expression) =>
            {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            _ => return None,
        }
    }
    Some(has_typed_operand)
}
