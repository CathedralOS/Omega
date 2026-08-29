//! Canonical abstract-operation and scalar carrier encoding.

use super::structural_encoding::*;
use super::*;

pub(super) fn encode_operation(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::EstablishPayloadlessCase {
            psi_operation,
            result,
            result_case,
        } => {
            bytes.u8(48);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*result_case);
        }
        O::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes: literal,
        } => {
            bytes.u8(1);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
            bytes.len(literal.len());
            bytes.bytes(literal);
        }
        O::EstablishTrivialAffineLocal {
            psi_operation,
            place,
            structural_type,
        } => {
            bytes.u8(2);
            bytes.id(*psi_operation);
            encode_place_declaration(bytes, *place);
            encode_structural_type(bytes, structural_type);
        }
        O::CallUnit {
            psi_operation,
            callee,
            structural_arguments,
            claim_transfers,
        } => {
            bytes.u8(3);
            bytes.id(*psi_operation);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
        }
        O::CallStructuralScalar {
            psi_operation,
            result,
            callee,
            structural_arguments,
            claim_transfers,
        } => {
            bytes.u8(4);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
        }
        O::CallStructural {
            psi_operation,
            result,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => {
            bytes.u8(5);
            bytes.id(*psi_operation);
            encode_structural_operation_result(bytes, result);
            bytes.id(*callee);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(claim_transfers, |bytes, transfer| {
                bytes.id(transfer.claim);
                bytes.u32(transfer.argument_index);
            });
            bytes.slice(returned_claim_transfers, |bytes, transfer| {
                bytes.id(transfer.callee_claim);
                bytes.id(transfer.caller_claim);
            });
            encode_ids(bytes, requirement_obligations);
            bytes.slice(crash_continuations, encode_crash_route_bucket);
            encode_optional(bytes, selected_evidence.as_ref(), |bytes, evidence| {
                encode_outcome_specific_call_evidence(bytes, evidence)
            });
        }
        O::BoundaryCall {
            psi_operation,
            result,
            boundary,
            arguments,
            structural_arguments,
            completion_claim_sources,
            completion_receipts,
        } => {
            bytes.u8(6);
            bytes.id(*psi_operation);
            encode_optional(bytes, result.as_ref(), |bytes, result| {
                encode_abstract_result(bytes, *result)
            });
            bytes.id(*boundary);
            encode_ids(bytes, arguments);
            bytes.slice(structural_arguments, encode_structural_argument);
            bytes.slice(completion_claim_sources, encode_completion_claim_source);
            bytes.slice(completion_receipts, |bytes, receipt| {
                bytes.id(receipt.claim);
                bytes.u32(receipt.argument_index);
            });
        }
        O::PortWrite {
            psi_operation,
            service,
            port,
            value,
        } => {
            bytes.u8(7);
            bytes.id(*psi_operation);
            bytes.id(*service);
            bytes.u16(*port);
            bytes.u8(*value);
        }
        O::Call {
            psi_operation,
            result,
            scalar_type,
            callee,
            arguments,
        } => {
            bytes.u8(8);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            bytes.id(*callee);
            encode_ids(bytes, arguments);
        }
        O::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => {
            bytes.u8(9);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *scalar_type);
            encode_integer_value(bytes, *value);
        }
        O::BooleanConstant {
            psi_operation,
            result,
            value,
        } => {
            bytes.u8(10);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.boolean(*value);
        }
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            field,
        } => {
            bytes.u8(11);
            bytes.id(*psi_operation);
            bytes.id(*result);
            bytes.id(*source);
            bytes.id(*field);
        }
        O::BooleanNot {
            psi_operation,
            result,
            operand,
        } => encode_untyped_unary(bytes, 12, *psi_operation, *result, *operand),
        O::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 13, *psi_operation, *result, *left, *right),
        O::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 14, *psi_operation, *result, *left, *right),
        O::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 15, *psi_operation, *result, *left, *right),
        O::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => encode_untyped_binary(bytes, 16, *psi_operation, *result, *left, *right),
        O::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => encode_typed_unary(bytes, 17, *psi_operation, *result, *scalar_type, *operand),
        O::IntegerWiden {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => encode_cast(
            bytes,
            18,
            *psi_operation,
            None,
            *result,
            *source_type,
            *target_type,
            *operand,
        ),
        O::IntegerExactCast {
            psi_operation,
            obligation,
            result,
            source_type,
            target_type,
            operand,
        } => encode_cast(
            bytes,
            19,
            *psi_operation,
            Some(*obligation),
            *result,
            *source_type,
            *target_type,
            *operand,
        ),
        O::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            20,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            21,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            22,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            23,
            *psi_operation,
            None,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            24,
            *psi_operation,
            None,
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            25,
            *psi_operation,
            Some(*obligation),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => encode_shift(
            bytes,
            26,
            *psi_operation,
            Some(*obligation),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            27,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerAdd {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            28,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            29,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            30,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerSubtract {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            31,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            32,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            33,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerMultiply {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            34,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            35,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            36,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            37,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            38,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            39,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            40,
            *psi_operation,
            Some(*obligation),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            41,
            *psi_operation,
            None,
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::Jump {
            psi_edge,
            target,
            bindings,
            trivial_affine_discards,
        } => {
            bytes.u8(42);
            bytes.id(*psi_edge);
            bytes.id(*target);
            bytes.slice(bindings, encode_binding);
            encode_ids(bytes, trivial_affine_discards);
        }
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            bytes.u8(43);
            bytes.id(*condition);
            encode_successor(bytes, when_true);
            encode_successor(bytes, when_false);
        }
        O::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        } => {
            bytes.u8(44);
            bytes.id(*psi_edge);
            bytes.id(*result);
            bytes.id(*value);
            encode_scalar_type(bytes, *scalar_type);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ReturnUnit {
            psi_edge,
            cleanup_actions,
        } => {
            bytes.u8(45);
            bytes.id(*psi_edge);
            bytes.slice(cleanup_actions, encode_cleanup);
        }
        O::ReturnStructural {
            psi_edge,
            source,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
        } => {
            bytes.u8(46);
            bytes.id(*psi_edge);
            bytes.id(*source);
            encode_ids(bytes, returned_claims);
            bytes.len(trivial_affine_locals.len());
            for (operation, place, structural_type) in trivial_affine_locals {
                bytes.id(*operation);
                encode_place_declaration(bytes, *place);
                encode_structural_type(bytes, structural_type);
            }
            encode_ids(bytes, trivial_affine_discards);
        }
        O::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => {
            bytes.u8(47);
            bytes.id(*psi_edge);
            encode_crash_cause(bytes, *cause);
            bytes.slice(site_guard, encode_crash_predicate);
            encode_ids(bytes, frontier_lower_bound);
        }
    }
}

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

pub(super) fn encode_binding(bytes: &mut CanonicalBytes, binding: &ValueBinding) {
    bytes.id(binding.parameter);
    bytes.id(binding.argument);
    encode_scalar_type(bytes, binding.scalar_type);
}

pub(super) fn encode_successor(bytes: &mut CanonicalBytes, successor: &AbstractSuccessor) {
    bytes.id(successor.psi_edge);
    bytes.id(successor.target);
    bytes.slice(&successor.bindings, encode_binding);
    encode_ids(bytes, &successor.trivial_affine_discards);
}

pub(super) fn encode_optional<T>(
    bytes: &mut CanonicalBytes,
    value: Option<&T>,
    encode: impl Fn(&mut CanonicalBytes, &T),
) {
    bytes.boolean(value.is_some());
    if let Some(value) = value {
        encode(bytes, value);
    }
}

pub(super) fn encode_ids<T: PsiSemanticId>(bytes: &mut CanonicalBytes, ids: &[T]) {
    bytes.len(ids.len());
    for id in ids {
        bytes.id(*id);
    }
}

pub(super) fn encode_abstract_result(
    bytes: &mut CanonicalBytes,
    result: omega_abstract_operations::AbstractResult,
) {
    bytes.id(result.value);
    encode_scalar_type(bytes, result.scalar_type);
}

pub(super) fn encode_scalar_type(bytes: &mut CanonicalBytes, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.u8(1),
        ScalarType::Integer(integer) => {
            bytes.u8(2);
            encode_integer_type(bytes, integer);
        }
    }
}

pub(super) fn encode_integer_type(bytes: &mut CanonicalBytes, integer_type: IntegerType) {
    bytes.u8(match integer_type.sign() {
        IntegerSign::Unsigned => 1,
        IntegerSign::Signed => 2,
    });
    bytes.u16(integer_type.bits());
}

pub(super) fn encode_integer_value(bytes: &mut CanonicalBytes, value: IntegerValue) {
    match value {
        IntegerValue::Unsigned(value) => {
            bytes.u8(1);
            bytes.u128(value);
        }
        IntegerValue::Signed(value) => {
            bytes.u8(2);
            bytes.bytes(&value.to_le_bytes());
        }
    }
}

pub(super) fn encode_structural_parameter(
    bytes: &mut CanonicalBytes,
    parameter: &StructuralParameterDeclaration,
) {
    bytes.id(parameter.place);
    bytes.u32(parameter.position);
    bytes.boolean(parameter.is_self);
    bytes.id(parameter.structural_type);
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(bytes, &parameter.qualifications);
}

pub(super) fn encode_structural_argument(
    bytes: &mut CanonicalBytes,
    argument: &StructuralArgument,
) {
    bytes.id(argument.place);
    bytes.slice(&argument.path, encode_structural_path_segment);
    encode_access(bytes, argument.access);
}

pub(super) fn encode_structural_path_segment(
    bytes: &mut CanonicalBytes,
    segment: &StructuralPathSegment,
) {
    match segment {
        StructuralPathSegment::Field(identity) => {
            bytes.u8(1);
            bytes.string(identity);
        }
        StructuralPathSegment::FixedIndex(index) => {
            bytes.u8(2);
            bytes.u64(*index);
        }
    }
}

pub(super) fn encode_access(bytes: &mut CanonicalBytes, access: StructuralAccess) {
    bytes.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub(super) fn encode_multiplicity(
    bytes: &mut CanonicalBytes,
    multiplicity: StructuralMultiplicity,
) {
    bytes.u8(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}
