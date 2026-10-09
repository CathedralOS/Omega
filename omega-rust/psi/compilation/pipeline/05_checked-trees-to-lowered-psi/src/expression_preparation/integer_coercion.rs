//! Exact authored integer carrier changes, independent of result policy.
//!
//! Fixed-integer widening is total; partial conversion retains the source
//! checker's occurrence decision. An address keeps its own carrier and is
//! interpreted as a count only through its distance from address zero. That
//! established subtraction evaluates the original address once and retains
//! its independent formation obligation; it never grants address authority to
//! an ordinary integer. Address models wider than u64 need another denotation.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::emission::scalar_types::integer_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::types::PrimitiveType;
use semantic_vocabulary::{IntegerValue, ScalarType};

pub(crate) fn coerce_integer_carrier(
    operand: LoweredDirectExpression,
    target: ScalarType,
    formation_decided: bool,
) -> Result<LoweredDirectExpression, LoweringError> {
    let Some(source_type) = operand.scalar_type().as_integer() else {
        return unsupported("exact integer coercion requires an integer or address source");
    };
    let Some(target_type) = target.as_integer() else {
        return unsupported("exact integer coercion requires an integer destination");
    };
    if source_type == target_type {
        return Ok(operand);
    }
    if target_type.is_address() {
        return unsupported("exact integer coercion to an address has no admitted denotation");
    }
    let operand = if source_type.is_address() {
        if source_type.bits() > 64 {
            return unsupported("exact coercion from an address wider than the count carrier");
        }
        LoweredDirectExpression::IntegerBinary {
            kind: LoweredIntegerBinaryKind::ExactSubtract,
            scalar_type: integer_scalar_type(PrimitiveType::U64)?,
            left: Box::new(operand),
            right: Box::new(LoweredDirectExpression::IntegerLiteral {
                scalar_type: ScalarType::Integer(source_type),
                value: IntegerValue::Unsigned(0),
            }),
            formation_decided: false,
        }
    } else {
        operand
    };
    let Some(source_type) = operand.scalar_type().as_integer() else {
        return unsupported("integer coercion lost its integer carrier");
    };
    if source_type == target_type {
        Ok(operand)
    } else if source_type.can_widen_to(target_type) {
        Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type: target,
            operand: Box::new(operand),
        })
    } else {
        Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type: target,
            operand: Box::new(operand),
            formation_decided,
        })
    }
}
