//! Canonical field shapes shared by scalar operation variants.

use super::super::IntegerType;
use super::CanonicalBytes;
use crate::optimization_unit::identity::carrier_encoding::{encode_integer_type, encode_optional};
pub(super) fn encode_untyped_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    result: semantic_vocabulary::ValueId,
    operand: semantic_vocabulary::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    bytes.id(operand);
}

pub(super) fn encode_untyped_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    result: semantic_vocabulary::ValueId,
    left: semantic_vocabulary::ValueId,
    right: semantic_vocabulary::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    bytes.id(left);
    bytes.id(right);
}

pub(super) fn encode_typed_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    result: semantic_vocabulary::ValueId,
    scalar_type: IntegerType,
    operand: semantic_vocabulary::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    encode_integer_type(bytes, scalar_type);
    bytes.id(operand);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_cast(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    obligation: Option<semantic_vocabulary::ObligationId>,
    result: semantic_vocabulary::ValueId,
    source_type: IntegerType,
    target_type: IntegerType,
    operand: semantic_vocabulary::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
    bytes.id(result);
    encode_integer_type(bytes, source_type);
    encode_integer_type(bytes, target_type);
    bytes.id(operand);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_typed_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    obligation: Option<semantic_vocabulary::ObligationId>,
    result: semantic_vocabulary::ValueId,
    scalar_type: IntegerType,
    left: semantic_vocabulary::ValueId,
    right: semantic_vocabulary::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
    bytes.id(result);
    encode_integer_type(bytes, scalar_type);
    bytes.id(left);
    bytes.id(right);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_shift(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    obligation: Option<semantic_vocabulary::ObligationId>,
    result: semantic_vocabulary::ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value: semantic_vocabulary::ValueId,
    count: semantic_vocabulary::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    encode_optional(bytes, obligation.as_ref(), |bytes, value| bytes.id(*value));
    bytes.id(result);
    encode_integer_type(bytes, value_type);
    encode_integer_type(bytes, count_type);
    bytes.id(value);
    bytes.id(count);
}

/// A Trapping primitive: the tag, its site and result, both carrier axes,
/// then the primitive's own tag and operands in evaluation order.
pub(super) fn encode_trapping(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: semantic_vocabulary::OperationId,
    result: semantic_vocabulary::ValueId,
    scalar_type: IntegerType,
    operand_type: IntegerType,
    primitive: terminal_psi::TrappingIntegerOperation,
) {
    use terminal_psi::TrappingIntegerPrimitive as P;
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    encode_integer_type(bytes, scalar_type);
    encode_integer_type(bytes, operand_type);
    bytes.u8(match primitive.primitive() {
        P::Add => 0,
        P::Subtract => 1,
        P::Multiply => 2,
        P::Divide => 3,
        P::Remainder => 4,
        P::ShiftLeft => 5,
        P::ShiftRight => 6,
        P::Convert => 7,
    });
    for operand in primitive.operands() {
        bytes.id(operand);
    }
}
