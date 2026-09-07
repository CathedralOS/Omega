//! Anonymous numeric formation and typed integer-to-float boundaries.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

pub(in crate::literals) fn validate_numeric_tree_destination(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) {
        return;
    }
    let Some(has_typed_operand) = numeric_literal_tree(program, expression) else {
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
        if super::anonymous_exact_float_tree(program, expression).is_some() {
            return;
        }
        format!(
            "anonymous numeric arithmetic has no exact value for `{}` landing (division by zero is invalid)",
            destination.name()
        )
    };
    diagnostics.push(
        Diagnostic::error(message)
            .with_source_span(program.expression_table.source_span(expression)),
    );
}

/// Classify a closed builtin numeric tree. `Some(true)` contains an already
/// landed integer; typed floats and nonliteral operands decline this check.
pub(super) fn numeric_literal_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<bool> {
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
            ExpressionNode::Float(literal) if literal.landing().is_none() => {}
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

pub(crate) fn validate_anonymous_divisions(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Formation applies to retained source expressions, even when their value
    // is used only in a comparison or a proof and has no numeric destination.
    for (expression, node) in program.expression_table.expression_entries() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        if binary.operator != typed_trees::expression::BinaryOperator::Divide
            || numeric_literal_tree(program, expression) != Some(false)
        {
            continue;
        }
        if matches!(super::anonymous_exact_float_tree(program, binary.right),
            Some(numerics::bignum::ExactFloat::Finite(value)) if value.is_zero())
        {
            diagnostics.push(
                Diagnostic::error("anonymous division by zero has no exact numeric value")
                    .with_source_span(program.expression_table.source_span(expression)),
            );
        }
    }
}
