//! Unary operators: `!` on a Boolean and bitwise complement on an integer.

use super::unsigned_constants::bitwise_known_unsigned;
use super::{Analysis, BOOLEAN_INTERVAL, ExpressionWalk};
use crate::proof_contracts::arithmetic_domains::{Diagnostic, Interval, PrimitiveType, bitwise};
use typed_trees::expression::{TableUnaryExpression, UnaryOperator};

pub(super) fn analyze(
    walk: &ExpressionWalk,
    unary: &TableUnaryExpression,
    diagnostics: &mut Vec<Diagnostic>,
) -> Analysis {
    // Negation must not hide formation errors in its operand subtree.
    let operand = walk.analyze(unary.operand, diagnostics);
    match unary.operator {
        UnaryOperator::BitwiseNot => {
            let primitive = operand.primitive.or_else(|| {
                (operand.interval.low.is_some() && operand.interval.high.is_some())
                    .then_some(walk.target_primitive)
                    .flatten()
            });
            Analysis {
                domain: operand.domain,
                interval: primitive
                    .map(|primitive| {
                        bitwise::complement(
                            primitive,
                            operand.interval,
                            bitwise_known_unsigned(
                                walk.program,
                                walk.environment,
                                primitive,
                                unary.operand,
                                0,
                            ),
                        )
                    })
                    .unwrap_or(Interval::UNBOUNDED),
                primitive,
            }
        }
        UnaryOperator::LogicalNot => Analysis {
            domain: None,
            interval: BOOLEAN_INTERVAL,
            primitive: Some(PrimitiveType::Bool),
        },
    }
}
