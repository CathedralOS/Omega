//! Equality of fixed-carrier wrapping polynomials, independently reconstructed.
//!
//! Projection is private scratch for the quotient ring, never a mathematical
//! interpretation of the executable operands. Every node retains one exact
//! carrier and only wrapping add/subtract/multiply participates. Coefficients
//! reduce modulo 2^width; signed and unsigned canonical representatives of one
//! carrier have the same residue equality. No order or disequality follows.

use semantic_vocabulary::{IntegerMathTerm, IntegerType, ScalarTerm, ScalarType};

use super::{closed_integer::ClosedIntegerEvaluationError, open_terms};

const MAXIMUM_NODES: usize = 8_192;
const MAXIMUM_DEPTH: usize = 128;

pub(crate) fn equal(
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<bool, ClosedIntegerEvaluationError> {
    let ScalarType::Integer(carrier) = left.scalar_type() else {
        return Ok(false);
    };
    if carrier.is_address() || right.scalar_type() != ScalarType::Integer(carrier) {
        return Ok(false);
    }
    let mut remaining = MAXIMUM_NODES;
    let Some(left) = project(left, carrier, &mut remaining, 0)? else {
        return Ok(false);
    };
    let Some(right) = project(right, carrier, &mut remaining, 0)? else {
        return Ok(false);
    };
    open_terms::equal_modulo(&left, &right, carrier.bits())
}

fn project(
    term: &ScalarTerm,
    carrier: IntegerType,
    remaining: &mut usize,
    depth: usize,
) -> Result<Option<IntegerMathTerm>, ClosedIntegerEvaluationError> {
    if depth >= MAXIMUM_DEPTH || *remaining == 0 {
        return Err(ClosedIntegerEvaluationError::ResourceLimitExceeded);
    }
    *remaining -= 1;
    if term.scalar_type() != ScalarType::Integer(carrier) {
        return Ok(None);
    }
    let (left, right) = match term {
        ScalarTerm::Value { id, .. } => {
            return Ok(Some(IntegerMathTerm::MathValue {
                source_type: carrier,
                value: *id,
            }));
        }
        ScalarTerm::Integer { value, .. } if carrier.admits(*value) => {
            return Ok(Some(IntegerMathTerm::literal(*value)));
        }
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. } => (left, right),
        _ => return Ok(None),
    };
    let Some(left) = project(left, carrier, remaining, depth + 1)? else {
        return Ok(None);
    };
    let Some(right) = project(right, carrier, remaining, depth + 1)? else {
        return Ok(None);
    };
    let (left, right) = (Box::new(left), Box::new(right));
    Ok(Some(match term {
        ScalarTerm::WrappingIntegerAdd { .. } => IntegerMathTerm::Add(left, right),
        ScalarTerm::WrappingIntegerSubtract { .. } => IntegerMathTerm::Subtract(left, right),
        ScalarTerm::WrappingIntegerMultiply { .. } => IntegerMathTerm::Multiply(left, right),
        _ => unreachable!("projected wrapping ring operation"),
    }))
}

#[cfg(test)]
mod tests;
