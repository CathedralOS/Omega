//! Anonymous numeric formation is determined by resolved builtin operations
//! and exact literal values. Destination typing cannot supply a missing integer
//! operand or make division by zero well formed.

use crate::TypedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::types::PrimitiveType;
use diagnostics::Diagnostic;
use language_core::OperatorSpelling;

pub fn validate_numeric_tree_destination(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !matches!(destination, PrimitiveType::F32 | PrimitiveType::F64) {
        return;
    }
    let Some(has_typed_operand) = numeric_literal_tree(program, expression) else {
        return;
    };
    let message = if has_typed_operand {
        format!(
            "typed integer arithmetic cannot implicitly land in `{}`; use an explicit numeric conversion",
            destination.name()
        )
    } else {
        if anonymous_exact_float_tree(program, expression).is_some() {
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

pub fn numeric_literal_tree(program: &TypedTrees, expression: ExpressionHandle) -> Option<bool> {
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
                if crate::closed_numeric::has_anonymous_operator_meaning(program, expression) =>
            {
                pending.push(binary.left);
                pending.push(binary.right);
            }
            _ => return None,
        }
    }
    Some(has_typed_operand)
}

const REMAINDER_FORMATION_SPELLINGS: &[BinaryOperator] = &[BinaryOperator::Modulo];
const DIVISION_FORMATION_SPELLINGS: &[BinaryOperator] = &[BinaryOperator::Divide];
const BITWISE_FORMATION_SPELLINGS: &[BinaryOperator] = &[
    BinaryOperator::BitwiseAnd,
    BinaryOperator::BitwiseOr,
    BinaryOperator::BitwiseXor,
    BinaryOperator::ShiftLeft,
    BinaryOperator::ShiftRight,
];

const fn bitwise(operator: BinaryOperator) -> Option<(&'static str, bool)> {
    match operator {
        BinaryOperator::BitwiseAnd => Some(("&", false)),
        BinaryOperator::BitwiseOr => Some(("|", false)),
        BinaryOperator::BitwiseXor => Some(("^", false)),
        BinaryOperator::ShiftLeft => Some(("<<", true)),
        BinaryOperator::ShiftRight => Some((">>", true)),
        _ => None,
    }
}

fn anonymous_formation_message(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> Option<String> {
    let anonymous = |expression: ExpressionHandle| -> bool {
        numeric_literal_tree(program, expression) == Some(false)
    };
    match operator {
        BinaryOperator::Modulo => {
            let builtin = crate::closed_numeric::has_builtin_anonymous_operands(
                program,
                expression,
                OperatorSpelling::Modulo,
            );
            if !builtin || !anonymous(left) || !anonymous(right) {
                return None;
            }
            Some(
                "builtin `%` requires an integer-typed operand; type an operand (for example, \
     `7i32 % 2`), not just the destination"
                    .to_string(),
            )
        }
        BinaryOperator::Divide => {
            if !anonymous(expression) {
                return None;
            }
            matches!(
                anonymous_exact_float_tree(program, right),
                Some(numerics::bignum::ExactFloat::Finite(value)) if value.is_zero()
            )
            .then(|| "anonymous division by zero has no exact numeric value".to_string())
        }
        operator => {
            let (spelling, shifts) = bitwise(operator)?;
            if !anonymous(left) || !(shifts || anonymous(right)) {
                return None;
            }
            Some(if shifts {
                format!(
                    "builtin `{spelling}` takes its width from the value it shifts, so that value \
         needs an integer type; type it (for example, `4i32 {spelling} 1`), not just \
         the destination"
                )
            } else {
                format!(
                    "builtin `{spelling}` requires an integer-typed operand; type an operand (for \
         example, `6i32 {spelling} 4`), not just the destination"
                )
            })
        }
    }
}

fn report_anonymous_formation(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
    spellings: &[BinaryOperator],
) {
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        if !spellings.contains(&binary.operator) {
            continue;
        }
        let Some(message) = anonymous_formation_message(
            program,
            expression,
            binary.operator,
            binary.left,
            binary.right,
        ) else {
            continue;
        };
        diagnostics.push(
            Diagnostic::error(message)
                .with_source_span(program.expression_table.source_span(expression)),
        );
    }
}

pub fn validate_anonymous_numeric_formation(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    report_anonymous_formation(program, diagnostics, REMAINDER_FORMATION_SPELLINGS);
    report_anonymous_formation(program, diagnostics, DIVISION_FORMATION_SPELLINGS);
}

pub fn validate_anonymous_bitwise(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    report_anonymous_formation(program, diagnostics, BITWISE_FORMATION_SPELLINGS);
}

pub fn anonymous_exact_float_tree(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::ExactFloat> {
    use crate::expression::BinaryOperator;

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) if literal.landing().is_none() => {
            Some(numerics::bignum::ExactFloat::Finite(
                numerics::bignum::BigRational::from_integer(literal.value_bignum()?),
            ))
        }
        ExpressionNode::Float(literal) if literal.landing().is_none() => {
            numerics::bignum::ExactFloat::from_decimal_str(literal.text())
        }
        ExpressionNode::Borrow(inner) => anonymous_exact_float_tree(program, inner.target),
        ExpressionNode::Binary(binary)
            if crate::closed_numeric::has_anonymous_operator_meaning(program, expression) =>
        {
            let left = anonymous_exact_float_tree(program, binary.left)?;
            let right = anonymous_exact_float_tree(program, binary.right)?;
            Some(match binary.operator {
                BinaryOperator::Add => left.add(&right),
                BinaryOperator::Subtract => left.sub(&right),
                BinaryOperator::Multiply => left.mul(&right),
                BinaryOperator::Divide => {
                    if matches!(&right, numerics::bignum::ExactFloat::Finite(value) if value.is_zero())
                    {
                        return None;
                    }
                    left.div(&right)
                }
                _ => return None,
            })
        }
        _ => None,
    }
}
