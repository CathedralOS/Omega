//! Canonical field shapes shared by scalar operation variants.

use super::*;

pub(super) fn encode_untyped_unary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    operand: psi_core::ValueId,
) {
    bytes.u8(tag);
    bytes.id(operation);
    bytes.id(result);
    bytes.id(operand);
}

pub(super) fn encode_untyped_binary(
    bytes: &mut CanonicalBytes,
    tag: u8,
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
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
    operation: psi_core::OperationId,
    result: psi_core::ValueId,
    scalar_type: IntegerType,
    operand: psi_core::ValueId,
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
    operation: psi_core::OperationId,
    obligation: Option<psi_core::ObligationId>,
    result: psi_core::ValueId,
    source_type: IntegerType,
    target_type: IntegerType,
    operand: psi_core::ValueId,
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
    operation: psi_core::OperationId,
    obligation: Option<psi_core::ObligationId>,
    result: psi_core::ValueId,
    scalar_type: IntegerType,
    left: psi_core::ValueId,
    right: psi_core::ValueId,
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
    operation: psi_core::OperationId,
    obligation: Option<psi_core::ObligationId>,
    result: psi_core::ValueId,
    value_type: IntegerType,
    count_type: IntegerType,
    value: psi_core::ValueId,
    count: psi_core::ValueId,
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
