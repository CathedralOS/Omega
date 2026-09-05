//! Canonical field shapes shared by scalar operation variants.

use super::*;

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
