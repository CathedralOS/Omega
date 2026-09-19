//! Canonical terminal block, operation, and terminator wire format.
//!
//! This module owns operation-result and operation-kind rows plus the exact
//! terminal control-flow envelope. Shared structural paths, call arguments,
//! contracts, and declaration primitives remain sibling- or parent-owned.
//!
//! `encode_block` and `decode_block` are the entries. Each operation row is
//! encoded and decoded here, with its kind dispatched by `operation_tags`
//! to the family that owns the layout (`value_operations`,
//! `storage_operations`, `call_operations`, `scalar_operations`); the
//! terminator envelope is `terminator_wire`.

mod call_operations;
mod operation_tags;
mod scalar_operations;
mod storage_operations;
mod terminator_wire;
mod value_operations;

use semantic_vocabulary::CanonicalStructuralPathSegment;
use terminal_psi::{Block, Operation, OperationKind, OperationResult};

use super::CodecError;
use super::machine_wire::{
    decode_declaration, decode_declarations, encode_declaration, encode_declarations,
};
use super::structural_result_wire::{decode_operation_result, encode_operation_result};
use super::structural_signature_wire::{
    decode_structural_parameters, encode_structural_parameters,
};
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;

fn encode_scalar_field_path(
    writer: &mut Writer,
    path: &[CanonicalStructuralPathSegment],
) -> Result<(), CodecError> {
    writer.len("scalar field carrier path", path.len())?;
    for segment in path {
        let CanonicalStructuralPathSegment::Field(field) = segment else {
            return Err(CodecError::MalformedStructuralFoundation(
                "scalar field carrier path is not a record path",
            ));
        };
        writer.u8(1);
        writer.id(*field);
    }
    Ok(())
}

fn decode_scalar_field_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<CanonicalStructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(CanonicalStructuralPathSegment::Field(
            reader.id("StructuralFieldId")?,
        )),
        tag => Err(CodecError::InvalidTag("scalar field carrier path", tag)),
    })
}

pub(crate) fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    encode_structural_parameters(writer, &block.structural_parameters)?;
    writer.len("operations", block.operations.len())?;
    for operation in &block.operations {
        encode_operation(writer, operation)?;
    }
    terminator_wire::encode_terminator(writer, &block.terminator)
}

/// One operation row: its id, static reach binding and result, then its
/// kind behind the kind's tag; each kind's layout is owned by its family.
fn encode_operation(writer: &mut Writer, operation: &Operation) -> Result<(), CodecError> {
    writer.id(operation.id);
    writer.boolean(operation.static_reach_binding.is_some());
    if let Some(binder) = operation.static_reach_binding {
        writer.u32(binder);
    }
    match &operation.result {
        OperationResult::Unit => writer.u8(0),
        OperationResult::Scalar(result) => {
            writer.u8(1);
            encode_declaration(writer, *result);
        }
        OperationResult::Structural(result) => {
            writer.u8(2);
            encode_operation_result(writer, result)?;
        }
    }
    match operation.kind.clone() {
        OperationKind::EstablishReference { source } => {
            storage_operations::encode_establish_reference(writer, source)?
        }
        OperationKind::ReleaseReference { source } => {
            storage_operations::encode_release_reference(writer, source)?
        }
        OperationKind::EstablishPrimitiveLocal { value } => {
            value_operations::encode_establish_primitive_local(writer, value)?
        }
        OperationKind::PrimitiveScalarRead { source, path } => {
            storage_operations::encode_primitive_scalar_read(writer, source, path)?
        }
        OperationKind::StructuralCaseMembership { source, path, case } => {
            storage_operations::encode_structural_case_membership(writer, source, path, case)?
        }
        OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            obligation,
        } => storage_operations::encode_byte_sequence_subslice(
            writer, source, start, end, length, obligation,
        )?,
        OperationKind::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            obligation,
        } => storage_operations::encode_byte_sequence_write(
            writer,
            destination,
            index,
            value,
            length,
            obligation,
        )?,
        OperationKind::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
        } => storage_operations::encode_byte_sequence_read(
            writer, source, index, length, obligation,
        )?,
        OperationKind::ByteSequenceLength { source } => {
            storage_operations::encode_byte_sequence_length(writer, source)?
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            path,
        } => {
            storage_operations::encode_write_only_primitive_store(writer, destination, value, path)?
        }
        OperationKind::WriteOnlyIndexedPrimitiveStore {
            destination,
            path,
            index,
            value,
            obligation,
        } => storage_operations::encode_write_only_indexed_primitive_store(
            writer,
            destination,
            path,
            index,
            value,
            obligation,
        )?,
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            length,
            obligation,
        } => storage_operations::encode_structural_byte_sequence_field_store(
            writer,
            destination,
            path,
            field,
            source,
            length,
            obligation,
        )?,
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } => storage_operations::encode_structural_byte_sequence_field_length(
            writer, source, path, field,
        )?,
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            index,
            value,
            length,
            obligation,
        } => storage_operations::encode_structural_byte_sequence_field_byte_store(
            writer,
            destination,
            path,
            field,
            index,
            value,
            length,
            obligation,
        )?,
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            range_obligation,
        } => storage_operations::encode_structural_scalar_field_store(
            writer,
            destination,
            path,
            field,
            value,
            range_obligation,
        )?,
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => storage_operations::encode_move_structural_field(writer, source, path, field)?,
        OperationKind::StoreStructuralField {
            destination,
            path,
            field,
            value,
        } => storage_operations::encode_store_structural_field(
            writer,
            destination,
            path,
            field,
            value,
        )?,
        OperationKind::EstablishScalarArray { elements } => {
            value_operations::encode_establish_scalar_array(writer, elements)?
        }
        OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } => value_operations::encode_establish_scalar_case(writer, result_case, fields)?,
        OperationKind::EstablishByteSequenceLiteral { destination, bytes } => {
            value_operations::encode_establish_byte_sequence_literal(writer, destination, bytes)?
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            value_operations::encode_establish_trivial_affine_local(writer, destination)?
        }
        OperationKind::EstablishRecord { fields } => {
            value_operations::encode_establish_record(writer, fields)?
        }
        OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
            storage_operations::encode_store_dynamic_descriptor(writer, descriptor_ordinal)?
        }
        OperationKind::Call {
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call(
            writer,
            callee,
            arguments,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallUnit {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_unit(
            writer,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallStructuralScalar {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_structural_scalar(
            writer,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallDynamicScalar {
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_dynamic_scalar(
            writer,
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallDynamicParameterScalar {
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_dynamic_parameter_scalar(
            writer,
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallDynamicUnit {
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_dynamic_unit(
            writer,
            descriptor_ordinal,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallDynamicParameterUnit {
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_dynamic_parameter_unit(
            writer,
            parameter_ordinal,
            requirement_slot,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => call_operations::encode_call_structural(
            writer,
            callee,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        )?,
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_structural_with_scalar_arguments(
            writer,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
        } => call_operations::encode_boundary_call(
            writer,
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
        )?,
        OperationKind::PortWrite {
            service,
            port,
            value,
        } => call_operations::encode_port_write(writer, service, port, value)?,
        OperationKind::IntegerConstant { value } => {
            value_operations::encode_integer_constant(writer, value)?
        }
        OperationKind::BooleanConstant { value } => {
            value_operations::encode_boolean_constant(writer, value)?
        }
        OperationKind::IeeeFloatConstant { value } => {
            value_operations::encode_ieee_float_constant(writer, value)?
        }
        OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        } => scalar_operations::encode_ieee_float_compare(writer, comparison, left, right)?,
        OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } => scalar_operations::encode_nearest_ieee_float_fused_multiply_add(
            writer, left, right, addend,
        )?,
        OperationKind::BooleanStructuralField {
            source,
            ref path,
            field,
        } => storage_operations::encode_boolean_structural_field(writer, source, path, field)?,
        OperationKind::IntegerStructuralField {
            source,
            ref path,
            field,
        } => storage_operations::encode_integer_structural_field(writer, source, path, field)?,
        OperationKind::BooleanNot { operand } => {
            scalar_operations::encode_boolean_not(writer, operand)?
        }
        OperationKind::BooleanEqual { left, right } => {
            scalar_operations::encode_boolean_equal(writer, left, right)?
        }
        OperationKind::IntegerEqual { left, right } => {
            scalar_operations::encode_integer_equal(writer, left, right)?
        }
        OperationKind::IntegerLessThan { left, right } => {
            scalar_operations::encode_integer_less_than(writer, left, right)?
        }
        OperationKind::IntegerLessOrEqual { left, right } => {
            scalar_operations::encode_integer_less_or_equal(writer, left, right)?
        }
        OperationKind::IntegerBitwiseNot { operand } => {
            scalar_operations::encode_integer_bitwise_not(writer, operand)?
        }
        OperationKind::IntegerWiden { operand } => {
            scalar_operations::encode_integer_widen(writer, operand)?
        }
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        } => scalar_operations::encode_integer_exact_cast(writer, operand, obligation)?,
        OperationKind::IntegerBitwiseAnd { left, right } => {
            scalar_operations::encode_integer_bitwise_and(writer, left, right)?
        }
        OperationKind::IntegerBitwiseOr { left, right } => {
            scalar_operations::encode_integer_bitwise_or(writer, left, right)?
        }
        OperationKind::IntegerBitwiseXor { left, right } => {
            scalar_operations::encode_integer_bitwise_xor(writer, left, right)?
        }
        OperationKind::WrappingIntegerShiftLeft { value, count } => {
            scalar_operations::encode_wrapping_integer_shift_left(writer, value, count)?
        }
        OperationKind::WrappingIntegerShiftRight { value, count } => {
            scalar_operations::encode_wrapping_integer_shift_right(writer, value, count)?
        }
        OperationKind::ExactIntegerShiftLeft {
            value,
            count,
            obligation,
        } => scalar_operations::encode_exact_integer_shift_left(writer, value, count, obligation)?,
        OperationKind::ExactIntegerShiftRight {
            value,
            count,
            obligation,
        } => scalar_operations::encode_exact_integer_shift_right(writer, value, count, obligation)?,
        OperationKind::ExactIntegerAdd {
            left,
            right,
            obligation,
        } => scalar_operations::encode_exact_integer_add(writer, left, right, obligation)?,
        OperationKind::ExactIntegerSubtract {
            left,
            right,
            obligation,
        } => scalar_operations::encode_exact_integer_subtract(writer, left, right, obligation)?,
        OperationKind::ExactIntegerMultiply {
            left,
            right,
            obligation,
        } => scalar_operations::encode_exact_integer_multiply(writer, left, right, obligation)?,
        OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        } => scalar_operations::encode_exact_integer_divide(writer, left, right, obligation)?,
        OperationKind::ExactIntegerRemainder {
            left,
            right,
            obligation,
        } => scalar_operations::encode_exact_integer_remainder(writer, left, right, obligation)?,
        OperationKind::WrappingIntegerDivide {
            left,
            right,
            obligation,
        } => scalar_operations::encode_wrapping_integer_divide(writer, left, right, obligation)?,
        OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        } => scalar_operations::encode_wrapping_integer_remainder(writer, left, right, obligation)?,
        OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        } => scalar_operations::encode_saturating_integer_divide(writer, left, right, obligation)?,
        OperationKind::SaturatingIntegerRemainder {
            left,
            right,
            obligation,
        } => {
            scalar_operations::encode_saturating_integer_remainder(writer, left, right, obligation)?
        }
        OperationKind::WrappingIntegerAdd { left, right } => {
            scalar_operations::encode_wrapping_integer_add(writer, left, right)?
        }
        OperationKind::SaturatingIntegerAdd { left, right } => {
            scalar_operations::encode_saturating_integer_add(writer, left, right)?
        }
        OperationKind::WrappingIntegerSubtract { left, right } => {
            scalar_operations::encode_wrapping_integer_subtract(writer, left, right)?
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            scalar_operations::encode_saturating_integer_subtract(writer, left, right)?
        }
        OperationKind::WrappingIntegerMultiply { left, right } => {
            scalar_operations::encode_wrapping_integer_multiply(writer, left, right)?
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            scalar_operations::encode_saturating_integer_multiply(writer, left, right)?
        }
    }
    Ok(())
}

pub(crate) fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let structural_parameters = decode_structural_parameters(reader)?;
    let operation_count = reader.count()?;
    let mut operations = Vec::new();
    for _ in 0..operation_count {
        operations.push(decode_operation(reader)?);
    }
    let terminator = terminator_wire::decode_terminator(reader)?;
    Ok(Block {
        id,
        parameters,
        structural_parameters,
        operations,
        terminator,
    })
}

/// One operation row: its id, static reach binding and result, then the
/// kind its tag names.
fn decode_operation(reader: &mut Reader<'_>) -> Result<Operation, CodecError> {
    let operation_id = reader.id("OperationId")?;
    let static_reach_binding = if reader.boolean()? {
        Some(reader.u32()?)
    } else {
        None
    };
    let result = match reader.u8()? {
        0 => OperationResult::Unit,
        1 => OperationResult::Scalar(decode_declaration(reader)?),
        2 => OperationResult::Structural(decode_operation_result(reader)?),
        tag => return Err(CodecError::InvalidTag("OperationResult", tag)),
    };
    let kind = match reader.u8()? {
        operation_tags::BYTE_SEQUENCE_SUBSLICE => {
            storage_operations::decode_byte_sequence_subslice(reader)?
        }
        operation_tags::BYTE_SEQUENCE_WRITE => {
            storage_operations::decode_byte_sequence_write(reader)?
        }
        operation_tags::BYTE_SEQUENCE_READ => {
            storage_operations::decode_byte_sequence_read(reader)?
        }
        operation_tags::BYTE_SEQUENCE_LENGTH => {
            storage_operations::decode_byte_sequence_length(reader)?
        }
        operation_tags::ESTABLISH_PRIMITIVE_LOCAL => {
            value_operations::decode_establish_primitive_local(reader)?
        }
        operation_tags::ESTABLISH_REFERENCE => {
            storage_operations::decode_establish_reference(reader)?
        }
        operation_tags::RELEASE_REFERENCE => storage_operations::decode_release_reference(reader)?,
        operation_tags::PRIMITIVE_SCALAR_READ => {
            storage_operations::decode_primitive_scalar_read(reader)?
        }
        operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ => {
            storage_operations::decode_projected_primitive_scalar_read(reader)?
        }
        operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_projected_write_only_primitive_store(reader)?
        }
        operation_tags::STRUCTURAL_CASE_MEMBERSHIP => {
            storage_operations::decode_structural_case_membership(reader)?
        }
        operation_tags::WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_write_only_primitive_store(reader)?
        }
        operation_tags::WRITE_ONLY_INDEXED_PRIMITIVE_STORE => {
            storage_operations::decode_write_only_indexed_primitive_store(reader)?
        }
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE => {
            storage_operations::decode_structural_byte_sequence_field_store(reader)?
        }
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH => {
            storage_operations::decode_structural_byte_sequence_field_length(reader)?
        }
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE => {
            storage_operations::decode_structural_byte_sequence_field_byte_store(reader)?
        }
        operation_tags::STRUCTURAL_SCALAR_FIELD_STORE => {
            storage_operations::decode_structural_scalar_field_store(reader)?
        }
        operation_tags::RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE => {
            storage_operations::decode_range_checked_structural_scalar_field_store(reader)?
        }
        operation_tags::MOVE_STRUCTURAL_FIELD => {
            storage_operations::decode_move_structural_field(reader)?
        }
        operation_tags::STORE_STRUCTURAL_FIELD => {
            storage_operations::decode_store_structural_field(reader)?
        }
        operation_tags::ESTABLISH_SCALAR_ARRAY => {
            value_operations::decode_establish_scalar_array(reader)?
        }
        operation_tags::ESTABLISH_SCALAR_CASE => {
            value_operations::decode_establish_scalar_case(reader)?
        }
        operation_tags::ESTABLISH_BYTE_SEQUENCE_LITERAL => {
            value_operations::decode_establish_byte_sequence_literal(reader)?
        }
        operation_tags::INTEGER_CONSTANT => value_operations::decode_integer_constant(reader)?,
        operation_tags::BOOLEAN_CONSTANT => value_operations::decode_boolean_constant(reader)?,
        operation_tags::IEEE_FLOAT_CONSTANT => {
            value_operations::decode_ieee_float_constant(reader)?
        }
        operation_tags::IEEE_FLOAT_COMPARE => scalar_operations::decode_ieee_float_compare(reader)?,
        operation_tags::NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD => {
            scalar_operations::decode_nearest_ieee_float_fused_multiply_add(reader)?
        }
        operation_tags::BOOLEAN_STRUCTURAL_FIELD => {
            storage_operations::decode_boolean_structural_field(reader)?
        }
        operation_tags::INTEGER_STRUCTURAL_FIELD => {
            storage_operations::decode_integer_structural_field(reader)?
        }
        operation_tags::WRAPPING_INTEGER_ADD => {
            scalar_operations::decode_wrapping_integer_add(reader)?
        }
        operation_tags::SATURATING_INTEGER_ADD => {
            scalar_operations::decode_saturating_integer_add(reader)?
        }
        operation_tags::WRAPPING_INTEGER_SUBTRACT => {
            scalar_operations::decode_wrapping_integer_subtract(reader)?
        }
        operation_tags::SATURATING_INTEGER_SUBTRACT => {
            scalar_operations::decode_saturating_integer_subtract(reader)?
        }
        operation_tags::WRAPPING_INTEGER_MULTIPLY => {
            scalar_operations::decode_wrapping_integer_multiply(reader)?
        }
        operation_tags::SATURATING_INTEGER_MULTIPLY => {
            scalar_operations::decode_saturating_integer_multiply(reader)?
        }
        operation_tags::BOOLEAN_NOT => scalar_operations::decode_boolean_not(reader)?,
        operation_tags::BOOLEAN_EQUAL => scalar_operations::decode_boolean_equal(reader)?,
        operation_tags::INTEGER_EQUAL => scalar_operations::decode_integer_equal(reader)?,
        operation_tags::INTEGER_LESS_THAN => scalar_operations::decode_integer_less_than(reader)?,
        operation_tags::INTEGER_LESS_OR_EQUAL => {
            scalar_operations::decode_integer_less_or_equal(reader)?
        }
        operation_tags::INTEGER_BITWISE_AND => {
            scalar_operations::decode_integer_bitwise_and(reader)?
        }
        operation_tags::INTEGER_BITWISE_OR => scalar_operations::decode_integer_bitwise_or(reader)?,
        operation_tags::INTEGER_BITWISE_XOR => {
            scalar_operations::decode_integer_bitwise_xor(reader)?
        }
        operation_tags::WRAPPING_INTEGER_SHIFT_LEFT => {
            scalar_operations::decode_wrapping_integer_shift_left(reader)?
        }
        operation_tags::WRAPPING_INTEGER_SHIFT_RIGHT => {
            scalar_operations::decode_wrapping_integer_shift_right(reader)?
        }
        operation_tags::INTEGER_BITWISE_NOT => {
            scalar_operations::decode_integer_bitwise_not(reader)?
        }
        operation_tags::INTEGER_WIDEN => scalar_operations::decode_integer_widen(reader)?,
        operation_tags::INTEGER_EXACT_CAST => scalar_operations::decode_integer_exact_cast(reader)?,
        operation_tags::EXACT_INTEGER_SHIFT_RIGHT => {
            scalar_operations::decode_exact_integer_shift_right(reader)?
        }
        operation_tags::EXACT_INTEGER_SHIFT_LEFT => {
            scalar_operations::decode_exact_integer_shift_left(reader)?
        }
        operation_tags::EXACT_INTEGER_ADD => scalar_operations::decode_exact_integer_add(reader)?,
        operation_tags::EXACT_INTEGER_SUBTRACT => {
            scalar_operations::decode_exact_integer_subtract(reader)?
        }
        operation_tags::EXACT_INTEGER_MULTIPLY => {
            scalar_operations::decode_exact_integer_multiply(reader)?
        }
        operation_tags::EXACT_INTEGER_DIVIDE => {
            scalar_operations::decode_exact_integer_divide(reader)?
        }
        operation_tags::EXACT_INTEGER_REMAINDER => {
            scalar_operations::decode_exact_integer_remainder(reader)?
        }
        operation_tags::WRAPPING_INTEGER_DIVIDE => {
            scalar_operations::decode_wrapping_integer_divide(reader)?
        }
        operation_tags::WRAPPING_INTEGER_REMAINDER => {
            scalar_operations::decode_wrapping_integer_remainder(reader)?
        }
        operation_tags::SATURATING_INTEGER_DIVIDE => {
            scalar_operations::decode_saturating_integer_divide(reader)?
        }
        operation_tags::SATURATING_INTEGER_REMAINDER => {
            scalar_operations::decode_saturating_integer_remainder(reader)?
        }
        operation_tags::CALL => call_operations::decode_call(reader)?,
        operation_tags::CALL_UNIT => call_operations::decode_call_unit(reader)?,
        operation_tags::BOUNDARY_CALL => call_operations::decode_boundary_call(reader)?,
        operation_tags::PORT_WRITE => call_operations::decode_port_write(reader)?,
        operation_tags::ESTABLISH_TRIVIAL_AFFINE_LOCAL => {
            value_operations::decode_establish_trivial_affine_local(reader)?
        }
        operation_tags::ESTABLISH_RECORD => value_operations::decode_establish_record(reader)?,
        operation_tags::CALL_STRUCTURAL_SCALAR => {
            call_operations::decode_call_structural_scalar(reader)?
        }
        operation_tags::CALL_DYNAMIC_SCALAR => call_operations::decode_call_dynamic_scalar(reader)?,
        operation_tags::CALL_DYNAMIC_PARAMETER_SCALAR => {
            call_operations::decode_call_dynamic_parameter_scalar(reader)?
        }
        operation_tags::CALL_DYNAMIC_UNIT => call_operations::decode_call_dynamic_unit(reader)?,
        operation_tags::CALL_DYNAMIC_PARAMETER_UNIT => {
            call_operations::decode_call_dynamic_parameter_unit(reader)?
        }
        operation_tags::STORE_DYNAMIC_DESCRIPTOR => {
            storage_operations::decode_store_dynamic_descriptor(reader)?
        }
        operation_tags::CALL_STRUCTURAL => call_operations::decode_call_structural(reader)?,
        operation_tags::CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS => {
            call_operations::decode_call_structural_with_scalar_arguments(reader)?
        }
        tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
    };
    Ok(Operation {
        static_reach_binding,
        id: operation_id,
        result,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{
        BlockId, CanonicalStructuralPathSegment, ClaimId, EdgeId, EvidenceTermId, IntegerSign,
        IntegerType, MachineId, ObligationId, OperationId, PlaceId, PropositionId, ScalarType,
        StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, EvidenceInterfaceIdentity, Operation, OperationKind, OperationResult,
        OutcomeSpecificCallEvidence, OutcomeSpecificCallEvidenceValidity, OutcomeSpecificGuard,
        StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
        StructuralPathSegment, StructuralResultClaimBinding, StructuralResultClaimTransfer,
        Terminator, ValueDeclaration,
    };

    use super::{decode_block, decode_scalar_field_path, encode_block, encode_scalar_field_path};
    use crate::{
        CodecError,
        sections::semantic_module::wire::{Reader, Writer},
    };

    fn id<T: semantic_vocabulary::PsiSemanticId>(raw: u64) -> T {
        T::new(raw).expect("test ids are nonzero")
    }

    fn jump_block(residual_affine_discards: Vec<terminal_psi::StructuralAffineDiscard>) -> Block {
        Block {
            structural_parameters: Vec::new(),
            id: id(1),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Jump {
                structural_arguments: Vec::new(),
                edge: id(2),
                target: id(3),
                arguments: vec![id(4)],
                trivial_affine_discards: vec![id(5)],
                residual_affine_discards,
            },
        }
    }

    #[test]
    fn root_only_jump_retains_its_exact_current_wire_encoding() {
        let block = jump_block(Vec::new());
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        let expected = [
            1_u64.to_le_bytes().as_slice(),
            0_u32.to_le_bytes().as_slice(),
            0_u32.to_le_bytes().as_slice(),
            0_u32.to_le_bytes().as_slice(),
            &[1],
            2_u64.to_le_bytes().as_slice(),
            3_u64.to_le_bytes().as_slice(),
            1_u32.to_le_bytes().as_slice(),
            4_u64.to_le_bytes().as_slice(),
            0_u32.to_le_bytes().as_slice(),
            1_u32.to_le_bytes().as_slice(),
            5_u64.to_le_bytes().as_slice(),
        ]
        .concat();
        assert_eq!(bytes, expected);
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(block));
    }

    #[test]
    fn scalar_field_carrier_wire_rejects_missing_steps_and_unsupported_tags() {
        let path = vec![CanonicalStructuralPathSegment::Field(
            id::<StructuralFieldId>(7),
        )];
        let mut writer = Writer::default();
        encode_scalar_field_path(&mut writer, &path).unwrap();
        let bytes = writer.finish();
        assert_eq!(decode_scalar_field_path(&mut Reader::new(&bytes)), Ok(path));
        for length in 0..bytes.len() {
            assert!(decode_scalar_field_path(&mut Reader::new(&bytes[..length])).is_err());
        }
        for tag in [0, 2, 3, 255] {
            let mut invalid = bytes.clone();
            invalid[4] = tag;
            assert!(
                matches!(decode_scalar_field_path(&mut Reader::new(&invalid)), Err(CodecError::InvalidTag("scalar field carrier path", actual)) if actual == tag)
            );
        }
    }

    #[test]
    fn residual_jump_round_trips_exact_ordered_paths_and_types() {
        let block = jump_block(vec![
            terminal_psi::StructuralAffineDiscard {
                place: id(6),
                path: vec![StructuralPathSegment::FixedIndex(2)],
                structural_type: id(7),
            },
            terminal_psi::StructuralAffineDiscard {
                place: id(6),
                path: vec![StructuralPathSegment::FixedIndex(0)],
                structural_type: id(7),
            },
        ]);
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[20], 10);
        let mut reader = Reader::new(&bytes);
        let decoded = decode_block(&mut reader).unwrap();
        assert_eq!(decoded, block);
        assert_eq!(reader.remaining(), 0);
        let mut writer = Writer::default();
        encode_block(&mut writer, &decoded).unwrap();
        assert_eq!(writer.finish(), bytes);
    }

    #[test]
    fn residual_jump_tag_rejects_an_empty_complement() {
        let mut writer = Writer::default();
        encode_block(&mut writer, &jump_block(Vec::new())).unwrap();
        let mut bytes = writer.finish();
        bytes[20] = 10;
        bytes.extend(0_u32.to_le_bytes());
        assert_eq!(
            decode_block(&mut Reader::new(&bytes)),
            Err(CodecError::NonCanonicalEncoding),
        );
    }

    fn structural_call_block() -> Block {
        Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(1),
                result: OperationResult::Structural(StructuralOperationResult {
                    place: id::<PlaceId>(2),
                    structural_type: id::<StructuralTypeId>(3),
                    multiplicity: StructuralMultiplicity::Linear,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: vec![StructuralResultClaimBinding {
                        claim: id::<ClaimId>(4),
                        path: Vec::new(),
                    }],
                }),
                kind: OperationKind::CallStructural {
                    callee: id::<MachineId>(5),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    returned_claim_transfers: vec![StructuralResultClaimTransfer {
                        callee_claim: id::<ClaimId>(6),
                        caller_claim: id::<ClaimId>(4),
                    }],
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(7),
                trivial_affine_discards: Vec::new(),
            },
        }
    }

    #[test]
    fn write_only_primitive_store_uses_exact_stable_wire_fields() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::WriteOnlyPrimitiveStore {
                    path: Vec::new(),
                    destination: id::<PlaceId>(3),
                    value: id(4),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(5),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("write-only primitive store block encodes");
        let bytes = writer.finish();
        assert_eq!(bytes[28], 0, "absent static reach binder marker");
        assert_eq!(bytes[29], 0, "Unit OperationResult wire tag");
        assert_eq!(bytes[30], 43, "WriteOnlyPrimitiveStore wire tag");
        assert_eq!(
            &bytes[31..39],
            &id::<PlaceId>(3).get().to_le_bytes(),
            "destination is the first exact operation field",
        );
        assert_eq!(
            &bytes[39..47],
            &id::<semantic_vocabulary::ValueId>(4).get().to_le_bytes(),
            "source value is the second exact operation field",
        );
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);

        let mut invalid = bytes;
        invalid[30] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid)),
            Err(CodecError::InvalidTag("OperationKind", 255)),
        );
    }

    #[test]
    fn structural_scalar_field_operations_use_exact_stable_wire_fields() {
        let store = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralScalarFieldStore {
                    destination: id::<PlaceId>(3),
                    path: vec![StructuralPathSegment::Field("item".into())],
                    field: id::<StructuralFieldId>(4),
                    value: id::<ValueId>(5),
                    range_obligation: None,
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(6),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &store).expect("structural scalar-field store encodes");
        let bytes = writer.finish();
        assert_eq!(bytes[30], 46, "StructuralScalarFieldStore wire tag");
        assert_eq!(&bytes[31..39], &id::<PlaceId>(3).get().to_le_bytes());
        assert_eq!(&bytes[39..43], &1_u32.to_le_bytes());
        assert_eq!(bytes[43], 1, "Field structural-path segment wire tag");
        assert_eq!(
            &bytes[52..60],
            &id::<StructuralFieldId>(4).get().to_le_bytes()
        );
        assert_eq!(&bytes[60..68], &id::<ValueId>(5).get().to_le_bytes());
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(store.clone()));

        let mut bounded = store;
        let OperationKind::StructuralScalarFieldStore {
            range_obligation, ..
        } = &mut bounded.operations[0].kind
        else {
            panic!("store");
        };
        *range_obligation = Some(id::<ObligationId>(7));
        let mut writer = Writer::default();
        encode_block(&mut writer, &bounded).expect("bounded store encodes");
        let bounded_bytes = writer.finish();
        assert_eq!(bounded_bytes[30], 75, "bounded scalar store extension tag");
        assert_eq!(&bounded_bytes[31..68], &bytes[31..68]);
        assert_eq!(
            &bounded_bytes[68..76],
            &id::<ObligationId>(7).get().to_le_bytes()
        );
        assert_eq!(decode_block(&mut Reader::new(&bounded_bytes)), Ok(bounded));

        let mut invalid_path = bytes;
        invalid_path[43] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid_path)),
            Err(CodecError::InvalidTag("StructuralPathSegment", 255)),
        );

        let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let read = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(7),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(8),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id::<ValueId>(9),
                    scalar_type: integer,
                }),
                kind: OperationKind::IntegerStructuralField {
                    path: Vec::new(),
                    source: id::<PlaceId>(10),
                    field: id::<StructuralFieldId>(11),
                },
            }],
            terminator: Terminator::Return {
                edge: id::<EdgeId>(12),
                value: id::<ValueId>(9),
                cleanup_actions: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &read).expect("integer structural field read encodes");
        let bytes = writer.finish();
        let kind = bytes
            .iter()
            .position(|byte| *byte == 47)
            .expect("IntegerStructuralField wire tag");
        assert_eq!(
            &bytes[kind + 1..kind + 9],
            &id::<PlaceId>(10).get().to_le_bytes()
        );
        assert_eq!(
            &bytes[kind + 13..kind + 21],
            &id::<StructuralFieldId>(11).get().to_le_bytes(),
        );
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(read));
    }

    #[test]
    fn byte_field_index_operations_roundtrip_exact_operands_and_paths() {
        let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let operations = [
            Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id::<ValueId>(3),
                    scalar_type: count_type,
                }),
                kind: OperationKind::StructuralByteSequenceFieldLength {
                    source: id::<PlaceId>(4),
                    path: vec![
                        StructuralPathSegment::Field("buffer".into()),
                        StructuralPathSegment::FixedIndex(2),
                    ],
                    field: id::<StructuralFieldId>(5),
                },
            },
            Operation {
                static_reach_binding: None,
                id: id::<OperationId>(6),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralByteSequenceFieldByteStore {
                    destination: id::<PlaceId>(7),
                    path: vec![
                        StructuralPathSegment::Field("nested".into()),
                        StructuralPathSegment::FixedIndex(3),
                    ],
                    field: id::<StructuralFieldId>(8),
                    index: id::<ValueId>(9),
                    value: id::<ValueId>(10),
                    length: id::<ValueId>(11),
                    obligation: id::<ObligationId>(12),
                },
            },
        ];
        for (operation, tag) in operations.into_iter().zip([59, 60]) {
            let block = Block {
                structural_parameters: Vec::new(),
                id: id::<BlockId>(1),
                parameters: Vec::new(),
                operations: vec![operation],
                terminator: Terminator::ReturnUnit {
                    edge: id::<EdgeId>(13),
                    trivial_affine_discards: Vec::new(),
                },
            };
            let mut writer = Writer::default();
            encode_block(&mut writer, &block).unwrap();
            let bytes = writer.finish();
            // Results follow the absent static reach marker; scalar declarations
            // also include the eight-byte qualification-set ID.
            let operation_tag_offset = if tag == 59 { 50 } else { 30 };
            assert_eq!(bytes[operation_tag_offset], tag);
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode_block(&mut reader), Ok(block));
            assert_eq!(reader.remaining(), 0);
            for prefix_length in 0..bytes.len() {
                assert!(decode_block(&mut Reader::new(&bytes[..prefix_length])).is_err());
            }
        }
    }

    #[test]
    fn primitive_local_wire_tags_and_identities_are_exact_and_truncation_rejects() {
        let operations = [
            Operation {
                static_reach_binding: None,
                id: id::<OperationId>(31),
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    place: id(23),
                    structural_type: id(7),
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::EstablishPrimitiveLocal { value: id(11) },
            },
            Operation {
                static_reach_binding: None,
                id: id::<OperationId>(32),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id(47),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::PrimitiveScalarRead {
                    path: Vec::new(),
                    source: id(23),
                },
            },
        ];
        // Empty block rosters precede the operation ID, absent static reach
        // marker, and typed result.
        for (operation, (tag, tag_offset, operand)) in operations
            .into_iter()
            .zip([(61, 59, 11_u64), (62, 47, 23_u64)])
        {
            let block = Block {
                id: id(1),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operations: vec![operation],
                terminator: Terminator::ReturnUnit {
                    edge: id(13),
                    trivial_affine_discards: Vec::new(),
                },
            };
            let mut writer = Writer::default();
            encode_block(&mut writer, &block).unwrap();
            let bytes = writer.finish();
            assert_eq!(bytes[tag_offset], tag);
            assert_eq!(
                &bytes[tag_offset + 1..tag_offset + 9],
                &operand.to_le_bytes()
            );
            let mut reader = Reader::new(&bytes);
            let decoded = decode_block(&mut reader).unwrap();
            assert_eq!(decoded, block);
            assert_eq!(reader.remaining(), 0);
            let mut writer = Writer::default();
            encode_block(&mut writer, &decoded).unwrap();
            assert_eq!(writer.finish(), bytes);
            for prefix_length in 0..bytes.len() {
                assert!(decode_block(&mut Reader::new(&bytes[..prefix_length])).is_err());
            }
            let mut zero_operand = bytes;
            zero_operand[tag_offset + 1..tag_offset + 9].fill(0);
            assert!(decode_block(&mut Reader::new(&zero_operand)).is_err());
        }
    }

    #[test]
    fn projected_primitive_wire_preserves_canonical_subject_and_rejects_truncation() {
        let path = vec![
            CanonicalStructuralPathSegment::Field(id(7)),
            CanonicalStructuralPathSegment::FixedIndex(255),
        ];
        let block = Block {
            id: id(1),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![
                Operation {
                    static_reach_binding: None,
                    id: id(1),
                    result: OperationResult::Unit,
                    kind: OperationKind::WriteOnlyPrimitiveStore {
                        destination: id(2),
                        path: path.clone(),
                        value: id(3),
                    },
                },
                Operation {
                    static_reach_binding: None,
                    id: id(2),
                    result: OperationResult::Scalar(ValueDeclaration {
                        qualifications: Default::default(),
                        id: id(4),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::PrimitiveScalarRead {
                        source: id(2),
                        path,
                    },
                },
            ],
            terminator: Terminator::ReturnUnit {
                edge: id(3),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        assert_eq!(decode_block(&mut Reader::new(&bytes)).unwrap(), block);
        for prefix in 0..bytes.len() {
            assert!(decode_block(&mut Reader::new(&bytes[..prefix])).is_err());
        }
        let mut changed = block.clone();
        let OperationKind::PrimitiveScalarRead { path, .. } = &mut changed.operations[1].kind
        else {
            panic!("read");
        };
        path[1] = CanonicalStructuralPathSegment::FixedIndex(254);
        let mut writer = Writer::default();
        encode_block(&mut writer, &changed).unwrap();
        assert_ne!(
            writer.finish(),
            bytes,
            "a different leaf has a different semantic encoding"
        );
    }

    #[test]
    fn byte_field_store_roundtrips_every_identity_and_rejects_truncation() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::StructuralByteSequenceFieldStore {
                    destination: id::<PlaceId>(3),
                    path: vec![
                        StructuralPathSegment::Field("item".into()),
                        StructuralPathSegment::FixedIndex(2),
                    ],
                    field: id::<StructuralFieldId>(4),
                    source: id::<PlaceId>(5),
                    length: id::<ValueId>(6),
                    obligation: id::<ObligationId>(7),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(8),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes[30], 58);
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);
        for length in 0..bytes.len() {
            assert!(decode_block(&mut Reader::new(&bytes[..length])).is_err());
        }
    }

    #[test]
    fn byte_sequence_length_wire_binds_exact_source_and_result() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id::<ValueId>(3),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                }),
                kind: OperationKind::ByteSequenceLength {
                    source: id::<PlaceId>(4),
                },
            }],
            terminator: Terminator::Return {
                edge: id::<EdgeId>(5),
                value: id::<ValueId>(3),
                cleanup_actions: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        let position = bytes
            .iter()
            .position(|byte| *byte == 55)
            .expect("length operation tag");
        assert_eq!(&bytes[position + 1..position + 9], &4_u64.to_le_bytes());
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(block.clone()));
        let mut changed = bytes.clone();
        changed[position + 1..position + 9].copy_from_slice(&6_u64.to_le_bytes());
        let decoded = decode_block(&mut Reader::new(&changed)).unwrap();
        assert_eq!(
            decoded.operations[0].kind,
            OperationKind::ByteSequenceLength {
                source: id::<PlaceId>(6)
            }
        );
        assert_eq!(decoded.operations[0].result, block.operations[0].result);
        changed[position + 1..position + 9].fill(0);
        assert!(decode_block(&mut Reader::new(&changed)).is_err());
        let mut unknown = bytes.clone();
        unknown[position] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&unknown)),
            Err(CodecError::InvalidTag("OperationKind", 255))
        );
        assert!(decode_block(&mut Reader::new(&bytes[..position + 8])).is_err());
    }

    #[test]
    fn byte_sequence_subslice_wire_binds_each_operand_and_structural_result() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                    place: id::<PlaceId>(9),
                    structural_type: id::<StructuralTypeId>(10),
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::ByteSequenceSubslice {
                    source: id::<PlaceId>(3),
                    start: id::<ValueId>(4),
                    end: id::<ValueId>(5),
                    length: id::<ValueId>(6),
                    obligation: id::<ObligationId>(7),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(8),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        let position = bytes.iter().position(|byte| *byte == 57).unwrap();
        for (operand, expected) in [3_u64, 4, 5, 6, 7].into_iter().enumerate() {
            let start = position + 1 + operand * 8;
            assert_eq!(&bytes[start..start + 8], &expected.to_le_bytes());
            let mut zero = bytes.clone();
            zero[start..start + 8].fill(0);
            assert!(decode_block(&mut Reader::new(&zero)).is_err());
            let mut changed = bytes.clone();
            changed[start..start + 8].copy_from_slice(&99_u64.to_le_bytes());
            let decoded = decode_block(&mut Reader::new(&changed)).unwrap();
            assert_ne!(decoded.operations[0].kind, block.operations[0].kind);
            assert_eq!(decoded.operations[0].result, block.operations[0].result);
        }
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(block.clone()));
        let mut unknown = bytes.clone();
        unknown[position] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&unknown)),
            Err(CodecError::InvalidTag("OperationKind", 255))
        );
        for length in 0..bytes.len() {
            assert!(decode_block(&mut Reader::new(&bytes[..length])).is_err());
        }
    }

    #[test]
    fn byte_sequence_write_wire_binds_all_operands_and_unit_result() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Unit,
                kind: OperationKind::ByteSequenceWrite {
                    destination: id::<PlaceId>(4),
                    index: id::<ValueId>(5),
                    value: id::<ValueId>(6),
                    length: id::<ValueId>(7),
                    obligation: id::<ObligationId>(8),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(9),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        let position = bytes.iter().position(|byte| *byte == 63).unwrap();
        for (operand, expected) in [4_u64, 5, 6, 7, 8].into_iter().enumerate() {
            let start = position + 1 + operand * 8;
            assert_eq!(&bytes[start..start + 8], &expected.to_le_bytes());
            let mut zero = bytes.clone();
            zero[start..start + 8].fill(0);
            assert!(decode_block(&mut Reader::new(&zero)).is_err());
            let mut changed = bytes.clone();
            changed[start..start + 8].copy_from_slice(&99_u64.to_le_bytes());
            let decoded = decode_block(&mut Reader::new(&changed)).unwrap();
            assert_ne!(decoded.operations[0].kind, block.operations[0].kind);
            assert_eq!(decoded.operations[0].result, OperationResult::Unit);
        }
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(block));
        let mut unknown = bytes.clone();
        unknown[position] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&unknown)),
            Err(CodecError::InvalidTag("OperationKind", 255))
        );
        for length in 0..bytes.len() {
            assert!(decode_block(&mut Reader::new(&bytes[..length])).is_err());
        }
    }

    #[test]
    fn byte_sequence_read_wire_binds_all_operands() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id::<ValueId>(3),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                }),
                kind: OperationKind::ByteSequenceRead {
                    source: id::<PlaceId>(4),
                    index: id::<ValueId>(5),
                    length: id::<ValueId>(6),
                    obligation: id::<ObligationId>(7),
                },
            }],
            terminator: Terminator::Return {
                edge: id::<EdgeId>(8),
                value: id::<ValueId>(3),
                cleanup_actions: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        let position = bytes.iter().position(|byte| *byte == 56).unwrap();
        for (operand, expected) in [4_u64, 5, 6, 7].into_iter().enumerate() {
            let start = position + 1 + operand * 8;
            assert_eq!(&bytes[start..start + 8], &expected.to_le_bytes());
            let mut zero = bytes.clone();
            zero[start..start + 8].fill(0);
            assert!(decode_block(&mut Reader::new(&zero)).is_err());
        }
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(block.clone()));
        let mut unknown = bytes.clone();
        unknown[position] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&unknown)),
            Err(CodecError::InvalidTag("OperationKind", 255))
        );
        assert!(decode_block(&mut Reader::new(&bytes[..position + 32])).is_err());
    }

    #[test]
    fn structural_scalar_call_round_trips_scalar_arguments() {
        let block = Block {
            structural_parameters: Vec::new(),
            id: id::<BlockId>(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                static_reach_binding: None,
                id: id::<OperationId>(2),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: id::<ValueId>(3),
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::CallStructuralScalar {
                    callee: id::<MachineId>(4),
                    arguments: vec![id::<ValueId>(5)],
                    structural_arguments: vec![StructuralArgument {
                        place: id::<PlaceId>(6),
                        path: Vec::new(),
                        access: StructuralAccess::SharedBorrow,
                    }],
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: id::<EdgeId>(7),
                trivial_affine_discards: Vec::new(),
            },
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("mixed structural scalar call encodes");
        let bytes = writer.finish();
        let decoded = decode_block(&mut Reader::new(&bytes)).expect("mixed call decodes");
        let OperationKind::CallStructuralScalar { arguments, .. } = &decoded.operations[0].kind
        else {
            unreachable!()
        };
        assert_eq!(arguments, &[id::<ValueId>(5)]);
        assert_eq!(decoded, block);
    }

    #[test]
    fn structural_operation_result_and_call_use_stable_wire_tags() {
        let block = structural_call_block();
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("structural call block encodes");
        let bytes = writer.finish();

        // Block id + scalar/structural parameter counts + operation count +
        // operation id + absent static reach marker.
        assert_eq!(bytes[28], 0, "absent static reach binder marker");
        assert_eq!(bytes[29], 2, "structural OperationResult wire tag");
        // The fixture has no qualifications and one whole-root claim, so the
        // operation-kind tag follows its fixed-width result metadata here.
        assert_eq!(bytes[71], 41, "CallStructural wire tag");

        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);

        let mut invalid_result = bytes.clone();
        invalid_result[29] = 3;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid_result)),
            Err(CodecError::InvalidTag("OperationResult", 3))
        );

        let mut invalid_call = bytes;
        invalid_call[71] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&invalid_call)),
            Err(CodecError::InvalidTag("OperationKind", 255))
        );
    }

    #[test]
    fn guarded_structural_call_selection_round_trips_exact_validity_carrier() {
        let mut block = structural_call_block();
        let OperationKind::CallStructural {
            selected_evidence, ..
        } = &mut block.operations[0].kind
        else {
            unreachable!()
        };
        selected_evidence.push(OutcomeSpecificCallEvidence {
            guard: OutcomeSpecificGuard {
                result_type: id::<StructuralTypeId>(3),
                result_case: id::<StructuralCaseId>(8),
            },
            position: 0,
            callee_obligation: id::<ObligationId>(9),
            callee_term: id::<EvidenceTermId>(10),
            output_field: "selected".into(),
            callee_proposition: id::<PropositionId>(11),
            instantiated_proposition: id::<PropositionId>(11),
            output: id::<EvidenceTermId>(12),
            result_substitution: None,
            validity: OutcomeSpecificCallEvidenceValidity {
                result: id::<PlaceId>(2),
                proposition_dependencies: vec![id::<PlaceId>(2)],
                evidence_interface: EvidenceInterfaceIdentity {
                    trait_identity: "ReadyEvidence".into(),
                    arguments: vec!["Outcome".into()],
                    requirements: Vec::new(),
                },
                interface_dependencies: Vec::new(),
            },
            expected_use_count: 0,
            uses: Vec::new(),
        });
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).expect("guarded call selection encodes");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_block(&mut reader), Ok(block));
        assert_eq!(reader.remaining(), 0);

        let mut truncated = bytes;
        truncated.pop();
        assert!(decode_block(&mut Reader::new(&truncated)).is_err());
    }
    #[test]
    fn record_wire_retains_operand_kinds_and_rejects_retired_constructor_tags() {
        let mut block = structural_call_block();
        let OperationResult::Structural(result) = &mut block.operations[0].result else {
            unreachable!()
        };
        result.multiplicity = StructuralMultiplicity::Affine;
        result.claims.clear();
        block.operations[0].kind = OperationKind::EstablishRecord {
            fields: vec![
                terminal_psi::RecordFieldInitializer {
                    field: id::<StructuralFieldId>(1),
                    value: terminal_psi::RecordFieldValue::Scalar {
                        value: id::<ValueId>(2),
                        range_obligation: Some(id::<ObligationId>(3)),
                    },
                },
                terminal_psi::RecordFieldInitializer {
                    field: id::<StructuralFieldId>(2),
                    value: terminal_psi::RecordFieldValue::Structural(
                        terminal_psi::StructuralArgument {
                            place: id::<PlaceId>(4),
                            path: Vec::new(),
                            access: terminal_psi::StructuralAccess::Owned,
                        },
                    ),
                },
            ],
        };
        let mut writer = Writer::default();
        encode_block(&mut writer, &block).unwrap();
        let bytes = writer.finish();
        // Claim-free structural result metadata ends after its three empty rosters.
        assert_eq!(bytes[59], 68);
        assert_eq!(decode_block(&mut Reader::new(&bytes)), Ok(block));
        for retired in [51, 67] {
            let mut changed = bytes.clone();
            changed[59] = retired;
            assert_eq!(
                decode_block(&mut Reader::new(&changed)),
                Err(CodecError::InvalidTag("OperationKind", retired))
            );
        }
        let mut changed = bytes.clone();
        changed[72] = 255;
        assert_eq!(
            decode_block(&mut Reader::new(&changed)),
            Err(CodecError::InvalidTag("RecordFieldValue", 255))
        );
        for length in 0..bytes.len() {
            assert!(decode_block(&mut Reader::new(&bytes[..length])).is_err());
        }
    }
}
