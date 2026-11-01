//! Exact authored integer carrier changes, independent of result policy.
//!
//! Fixed-integer widening is total; partial conversion retains the source
//! checker's occurrence decision. Count/address interpretation uses the same
//! exact conversion, with both carriers intact and no storage authority.
//! Width equality never replaces the source checker's symbolic fit proof.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::ScalarType;

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
    if !source_type.can_exact_cast_to(target_type) {
        return unsupported("exact integer coercion cannot change address models");
    }
    if source_type.can_widen_to(target_type) {
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
