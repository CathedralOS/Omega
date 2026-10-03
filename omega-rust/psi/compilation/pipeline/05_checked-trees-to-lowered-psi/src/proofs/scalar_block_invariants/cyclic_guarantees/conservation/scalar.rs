//! Simultaneously rebind scalar contract terms without changing their meaning.
//!
//! The operation constructor and every carrier are copied exactly. Storage
//! observations and values outside the invocation telescope cannot become
//! entry snapshots here. A candidate equality is only a search proposal: the
//! common roster must prove it at every arrival, including the initial one.

use std::collections::BTreeMap;

use semantic_vocabulary::{Proposition, ScalarTerm, ValueId};
use terminal_psi::ValueDeclaration;

pub(super) fn discover(
    term: &ScalarTerm,
    parameters: &[ValueDeclaration],
    bound: &BTreeMap<ValueId, ValueDeclaration>,
    remaining: &mut usize,
    candidates: &mut Vec<Proposition>,
    depth: usize,
) -> Option<ScalarTerm> {
    if depth >= 64 || !super::spend(remaining) {
        return None;
    }
    let mut child = |term| discover(term, parameters, bound, remaining, candidates, depth + 1);
    // Evaluate both children before testing whether their parent can be bound:
    // an unmapped result on one side must not hide an invocation-scoped term
    // on the other. No replacement is recursively rebound.
    macro_rules! binary {
        ($variant:ident, $left:expr, $right:expr $(, $field:ident)*) => {{
            let left = child($left);
            let right = child($right);
            ScalarTerm::$variant {
                left: Box::new(left?),
                right: Box::new(right?),
                $($field: *$field,)*
            }
        }};
    }
    macro_rules! unary {
        ($variant:ident, $operand:expr $(, $field:ident)*) => {
            ScalarTerm::$variant {
                operand: Box::new(child($operand)?),
                $($field: *$field,)*
            }
        };
    }
    macro_rules! shift {
        ($variant:ident, $value:expr, $count:expr, $value_type:ident, $count_type:ident) => {{
            let value = child($value);
            let count = child($count);
            ScalarTerm::$variant {
                value: Box::new(value?),
                count: Box::new(count?),
                value_type: *$value_type,
                count_type: *$count_type,
            }
        }};
    }
    let current = match term {
        ScalarTerm::Value { id, scalar_type } => {
            let formal = parameters.iter().find(|formal| formal.id == *id)?;
            let current = bound.get(id)?;
            if formal.scalar_type != *scalar_type || current.scalar_type != *scalar_type {
                return None;
            }
            ScalarTerm::value(current.id, *scalar_type)
        }
        ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } | ScalarTerm::IeeeFloatLiteral(_) => {
            term.clone()
        }
        ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::IeeeFloatField { .. }
        | ScalarTerm::ViewExtent { .. }
        | ScalarTerm::ByteSequencePredicate { .. } => return None,
        ScalarTerm::BooleanNot { operand } => unary!(BooleanNot, operand),
        ScalarTerm::BooleanEqual { left, right } => binary!(BooleanEqual, left, right),
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => binary!(IntegerEqual, left, right, scalar_type),
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => binary!(IntegerLessThan, left, right, scalar_type),
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => binary!(IntegerLessOrEqual, left, right, scalar_type),
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        } => binary!(IntegerBitwiseAnd, left, right, scalar_type),
        ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        } => binary!(IntegerBitwiseOr, left, right, scalar_type),
        ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => binary!(IntegerBitwiseXor, left, right, scalar_type),
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => binary!(ExactIntegerAdd, left, right, scalar_type),
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => binary!(ExactIntegerSubtract, left, right, scalar_type),
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => binary!(ExactIntegerMultiply, left, right, scalar_type),
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => binary!(ExactIntegerDivide, left, right, scalar_type),
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => binary!(ExactIntegerRemainder, left, right, scalar_type),
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => binary!(WrappingIntegerAdd, left, right, scalar_type),
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => binary!(WrappingIntegerSubtract, left, right, scalar_type),
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => binary!(WrappingIntegerMultiply, left, right, scalar_type),
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => binary!(WrappingIntegerDivide, left, right, scalar_type),
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => binary!(WrappingIntegerRemainder, left, right, scalar_type),
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => binary!(SaturatingIntegerAdd, left, right, scalar_type),
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => binary!(SaturatingIntegerSubtract, left, right, scalar_type),
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => binary!(SaturatingIntegerMultiply, left, right, scalar_type),
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => binary!(SaturatingIntegerDivide, left, right, scalar_type),
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => binary!(SaturatingIntegerRemainder, left, right, scalar_type),
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => unary!(IntegerBitwiseNot, operand, scalar_type),
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => unary!(IntegerWiden, operand, source_type, target_type),
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => unary!(IntegerExactCast, operand, source_type, target_type),
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => shift!(
            WrappingIntegerShiftLeft,
            value,
            count,
            value_type,
            count_type
        ),
        ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => shift!(
            WrappingIntegerShiftRight,
            value,
            count,
            value_type,
            count_type
        ),
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => shift!(ExactIntegerShiftLeft, value, count, value_type, count_type),
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => shift!(ExactIntegerShiftRight, value, count, value_type, count_type),
    };
    if current != *term {
        let predicate = if current < *term {
            Proposition::Equal(current.clone(), term.clone())
        } else {
            Proposition::Equal(term.clone(), current.clone())
        };
        if !candidates.contains(&predicate) {
            candidates.push(predicate);
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests;
