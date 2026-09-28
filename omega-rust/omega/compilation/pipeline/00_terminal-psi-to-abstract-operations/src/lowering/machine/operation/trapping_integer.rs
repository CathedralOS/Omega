//! A Terminal `Trapping` primitive lowers to the one abstract operation that
//! keeps its own `Trap` site. Lowering it as the Exact, Wrapping or Saturating
//! sibling would erase the trap, and expanding it into a compare, a branch and
//! a `Crash` terminator would fabricate a control edge Terminal never had, so
//! the primitive, its operands and both carrier axes cross the boundary intact
//! and native realization checks the predicate in place.
use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind, TrappingIntegerOperation};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    let OperationKind::TrappingInteger {
        operation: primitive,
    } = operation.kind
    else {
        unreachable!("the Trapping router dispatches only TrappingInteger");
    };
    let malformed = || LoweringError::VerifiedTrappingIntegerMalformed(operation.id);
    let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
        return Err(malformed());
    };
    let operand_type = |value| match value_types.get(&value) {
        Some(ScalarType::Integer(integer)) => Ok(*integer),
        _ => Err(malformed()),
    };
    // Binary arithmetic reads two values of the result carrier; a shift's
    // count and a conversion's source are independently typed.
    let operand_type = match primitive {
        TrappingIntegerOperation::Add { left, right }
        | TrappingIntegerOperation::Subtract { left, right }
        | TrappingIntegerOperation::Multiply { left, right }
        | TrappingIntegerOperation::Divide { left, right }
        | TrappingIntegerOperation::Remainder { left, right } => {
            if operand_type(left)? != scalar_type || operand_type(right)? != scalar_type {
                return Err(malformed());
            }
            scalar_type
        }
        TrappingIntegerOperation::ShiftLeft { value, count }
        | TrappingIntegerOperation::ShiftRight { value, count } => {
            if operand_type(value)? != scalar_type {
                return Err(malformed());
            }
            operand_type(count)?
        }
        TrappingIntegerOperation::Convert { operand } => operand_type(operand)?,
    };
    Ok(AbstractOperation::TrappingInteger {
        psi_operation: operation.id,
        result: operation.result.expect_scalar().id,
        scalar_type,
        operand_type,
        operation: primitive,
    })
}
