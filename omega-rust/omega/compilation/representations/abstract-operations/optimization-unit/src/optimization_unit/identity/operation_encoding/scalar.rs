//! Boolean and integer scalar-operation tags.

use crate::optimization_unit::identity::carrier_encoding::{
    encode_abstract_result, encode_integer_value, encode_scalar_type, encode_structural_argument,
    encode_structural_path_segment,
};
use crate::optimization_unit::identity::operation_encoding::scalar_shapes::{
    encode_cast, encode_shift, encode_trapping, encode_typed_binary, encode_typed_unary,
    encode_untyped_binary, encode_untyped_unary,
};
use crate::optimization_unit::identity::proposition_encoding::encode_canonical_path;

use super::{AbstractOperation, CanonicalBytes};
use abstract_operations::{
    IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy, IntegerShiftDirection,
    IntegerShiftPolicy,
};
use semantic_vocabulary::ScalarType;

pub(super) fn encode(bytes: &mut CanonicalBytes, operation: &AbstractOperation) {
    use AbstractOperation as O;
    match operation {
        O::StructuralByteSequenceFieldByteStore {
            psi_operation,
            destination,
            path,
            field,
            index,
            value,
            length,
            obligation,
        } => {
            bytes.u8(78);
            bytes.id(*psi_operation);
            bytes.id(*destination);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
            bytes.id(*index);
            bytes.id(*value);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::ByteSequenceStore {
            psi_operation,
            destination,
            source,
            length,
            obligation,
        } => {
            bytes.u8(241);
            bytes.id(*psi_operation);
            bytes.id(*destination);
            bytes.id(*source);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::StructuralByteSequenceFieldStore {
            psi_operation,
            destination,
            path,
            field,
            source,
            length,
            obligation,
        } => {
            bytes.u8(77);
            bytes.id(*psi_operation);
            bytes.id(*destination);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
            bytes.id(*source);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::StructuralByteViewFieldStore {
            psi_operation,
            destination,
            path,
            field,
            source,
            predicate_obligation,
        } => {
            bytes.u8(98);
            bytes.id(*psi_operation);
            bytes.id(*destination);
            bytes.slice(path, encode_structural_path_segment);
            super::encode_optional_field(bytes, *field);
            bytes.id(*source);
            match predicate_obligation {
                Some(obligation) => {
                    bytes.u8(1);
                    bytes.id(*obligation);
                }
                None => bytes.u8(0),
            }
        }
        O::ByteSequenceWrite {
            psi_operation,
            destination,
            index,
            value,
            length,
            obligation,
        } => {
            bytes.u8(68);
            bytes.id(*psi_operation);
            bytes.id(*destination);
            bytes.id(*index);
            bytes.id(*value);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::ByteSequenceRead {
            psi_operation,
            result,
            source,
            index,
            length,
            obligation,
        } => {
            bytes.u8(63);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            bytes.id(*index);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::ByteSequencePredicate {
            psi_operation,
            result,
            operand,
            predicate,
        } => {
            bytes.u8(100);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            encode_byte_sequence_operand(bytes, operand);
            bytes.u8(predicate.mask_bit());
        }
        O::ByteSequenceEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            bytes.u8(104);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            encode_byte_sequence_operand(bytes, left);
            encode_byte_sequence_operand(bytes, right);
        }
        O::StructuralByteSequenceFieldRead {
            psi_operation,
            result,
            source,
            path,
            field,
            index,
            length,
            obligation,
        } => {
            bytes.u8(89);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
            bytes.id(*index);
            bytes.id(*length);
            bytes.id(*obligation);
        }
        O::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            bytes.u8(76);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*field);
        }
        O::ByteSequenceLength {
            psi_operation,
            result,
            source,
        } => {
            bytes.u8(62);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            encode_structural_argument(bytes, source);
        }
        O::ElementViewLength {
            psi_operation,
            result,
            source,
        } => {
            bytes.u8(86);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            encode_structural_argument(bytes, source);
        }
        O::ElementViewRead {
            psi_operation,
            result,
            source,
            index,
            length,
            obligation,
            path,
        } => {
            bytes.u8(87);
            bytes.id(*psi_operation);
            encode_abstract_result(bytes, *result);
            bytes.id(*source);
            bytes.id(*index);
            bytes.id(*length);
            bytes.id(*obligation);
            bytes.slice(path, encode_structural_path_segment);
        }
        O::ElementViewWrite {
            path,
            psi_operation,
            destination,
            index,
            value,
            length,
            obligation,
        } => {
            bytes.u8(88);
            bytes.slice(path, encode_structural_path_segment);
            bytes.id(*psi_operation);
            encode_structural_argument(bytes, destination);
            bytes.id(*index);
            bytes.id(*value);
            bytes.id(*length);
            bytes.id(*obligation);
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
        // A Boolean read keeps tag 11 and its untyped result; any other
        // scalar keeps tag 51 and its typed result. These were the tags and
        // result layouts of the former Boolean and integer field-read
        // variants, so folding them into one operation moves no identity.
        O::StructuralScalarFieldRead {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            if result.scalar_type == ScalarType::Boolean {
                bytes.u8(11);
                bytes.id(*psi_operation);
                bytes.id(result.value);
            } else {
                bytes.u8(51);
                bytes.id(*psi_operation);
                encode_abstract_result(bytes, *result);
            }
            bytes.id(*source);
            encode_canonical_path(bytes, path);
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
        O::ScalarRepresentationView {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
        } => {
            // Tag 99 extends the roster. Both carriers are identity: the pair
            // decides how the result's register holds the same bits.
            bytes.u8(99);
            bytes.id(*psi_operation);
            bytes.id(*result);
            encode_scalar_type(bytes, *source_type);
            encode_scalar_type(bytes, *target_type);
            bytes.id(*operand);
        }
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
        O::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        } => encode_shift(
            bytes,
            integer_shift_tag(*policy, *direction),
            *psi_operation,
            policy.obligation(),
            *result,
            *value_type,
            *count_type,
            *value,
            *count,
        ),
        O::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            left,
            right,
        } => encode_typed_binary(
            bytes,
            integer_arithmetic_tag(*arithmetic),
            *psi_operation,
            arithmetic.obligation(),
            *result,
            *scalar_type,
            *left,
            *right,
        ),
        O::TrappingInteger {
            psi_operation,
            result,
            scalar_type,
            operand_type,
            operation,
        } => encode_trapping(
            bytes,
            90,
            *psi_operation,
            *result,
            *scalar_type,
            *operand_type,
            *operation,
        ),
        _ => unreachable!("operation family routing admitted a non-scalar operation"),
    }
}

/// The stable shift tag of each admitted policy and direction. The tags are
/// the ones the former per-pair variants carried; the obligation byte and
/// field order follow in `encode_shift`.
fn integer_shift_tag(policy: IntegerShiftPolicy, direction: IntegerShiftDirection) -> u8 {
    match (policy, direction) {
        (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Left) => 23,
        (IntegerShiftPolicy::Wrapping, IntegerShiftDirection::Right) => 24,
        (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Left) => 25,
        (IntegerShiftPolicy::Exact { .. }, IntegerShiftDirection::Right) => 26,
    }
}

/// The stable arithmetic tag of each admitted policy and operator. The tags
/// are the ones the former per-pair variants carried, so the table is not in
/// policy-major order: saturating multiply kept the last tag.
fn integer_arithmetic_tag(arithmetic: IntegerArithmetic) -> u8 {
    use IntegerArithmeticOperator as Operator;
    use IntegerOverflowPolicy as Policy;
    match (arithmetic.policy(), arithmetic.operator()) {
        (Policy::Wrapping, Operator::Add) => 27,
        (Policy::Exact, Operator::Add) => 28,
        (Policy::Saturating, Operator::Add) => 29,
        (Policy::Wrapping, Operator::Subtract) => 30,
        (Policy::Exact, Operator::Subtract) => 31,
        (Policy::Saturating, Operator::Subtract) => 32,
        (Policy::Wrapping, Operator::Multiply) => 33,
        (Policy::Exact, Operator::Multiply) => 34,
        (Policy::Exact, Operator::Divide) => 35,
        (Policy::Exact, Operator::Remainder) => 36,
        (Policy::Wrapping, Operator::Divide) => 37,
        (Policy::Wrapping, Operator::Remainder) => 38,
        (Policy::Saturating, Operator::Divide) => 39,
        (Policy::Saturating, Operator::Remainder) => 40,
        (Policy::Saturating, Operator::Multiply) => 41,
    }
}

fn encode_byte_sequence_operand(
    bytes: &mut CanonicalBytes,
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
) {
    match operand {
        terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } => {
            bytes.u8(0);
            encode_structural_argument(bytes, source);
            bytes.id(*length);
        }
        terminal_psi::control_flow::ByteSequenceOperand::Field {
            source,
            field,
            length,
        } => {
            bytes.u8(1);
            encode_structural_argument(bytes, source);
            bytes.id(*field);
            bytes.id(*length);
        }
    }
}
