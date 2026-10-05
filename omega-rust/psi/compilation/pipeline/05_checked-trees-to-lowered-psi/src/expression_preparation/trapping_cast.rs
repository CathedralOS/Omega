//! Trapping value conversion without erasing address identity.
//!
//! Terminal's trapping conversion consumes fixed integers. An address is not
//! a fixed integer even when their widths match. For a retained address model
//! no wider than the count carrier, its distance from address zero expresses
//! the same mathematical value as a count through ordinary exact subtraction.
//! That subtraction keeps both address operands and its own independently
//! verified representability obligation; the following conversion retains the
//! original Trap site and destination bounds. The source is evaluated once.
//!
//! Wider address models need a direct conversion denotation: narrowing their
//! distance to u64 would fail before the authored trapping conversion. Decline
//! those models rather than silently truncating or inventing a crash edge.

use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::integer::LoweredIntegerBinaryKind;
use crate::emission::scalar_types::integer_scalar_type;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::types::PrimitiveType;
use semantic_vocabulary::{IntegerValue, ScalarType};

pub(crate) fn trapping_cast(
    operand: LoweredDirectExpression,
    target: ScalarType,
) -> Result<LoweredDirectExpression, LoweringError> {
    let ScalarType::Integer(target_type) = target else {
        return unsupported("trapping conversion requires a fixed integer destination");
    };
    let ScalarType::Integer(source_type) = operand.scalar_type() else {
        return unsupported("trapping conversion requires an integer or address source");
    };
    if target_type.is_address() {
        return unsupported("trapping conversion to an address has no admitted denotation");
    }
    let operand = if source_type.is_address() {
        if source_type.bits() > 64 {
            return unsupported("trapping conversion from an address wider than the count carrier");
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
    Ok(LoweredDirectExpression::IntegerTrappingCast {
        scalar_type: target,
        operand: Box::new(operand),
    })
}
