//! Canonical terminal block, operation, and terminator wire format.
//!
//! This module owns operation-result and operation-kind rows plus the exact
//! terminal control-flow envelope. Shared structural paths, call arguments,
//! contracts, and declaration primitives remain sibling- or parent-owned.
//!
//! `encode_block` and `decode_block` are the entries. Each operation row is
//! encoded and decoded here, with its kind dispatched by `operation_tags`
//! to the family that owns the layout (`value_operations`,
//! `storage_operations`, `call_operations`, `scalar_operations`,
//! `atomic_operations`); the
//! terminator envelope is `terminator_wire`.

mod atomic_operations;
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
use super::proof_term_wire::{decode_proof_formals, encode_proof_formals};
use super::structural_field_wire::encode_canonical_structural_path;
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
    // One literal index may end the carrier when the element record owns the
    // observed field (`maps[1]` for `maps[1].value`). A final case step names
    // the payload namespace a case-qualified observation reads; foundation
    // validation and verification decide whether the carrier and case are admitted.
    encode_canonical_structural_path(writer, path, "scalar field carrier path")
}

fn decode_scalar_field_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<CanonicalStructuralPathSegment>, CodecError> {
    decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(CanonicalStructuralPathSegment::Field(
            reader.id("StructuralFieldId")?,
        )),
        2 => Ok(CanonicalStructuralPathSegment::FixedIndex(reader.u64()?)),
        3 => Ok(CanonicalStructuralPathSegment::Case(
            reader.id("StructuralCaseId")?,
        )),
        tag => Err(CodecError::InvalidTag("scalar field carrier path", tag)),
    })
}

pub(crate) fn encode_block(writer: &mut Writer, block: &Block) -> Result<(), CodecError> {
    writer.id(block.id);
    encode_declarations(writer, "block parameters", &block.parameters)?;
    encode_declarations(
        writer,
        "block erased scalar formals",
        &block.erased_scalar_formals,
    )?;
    encode_proof_formals(writer, &block.erased_proof_formals)?;
    encode_structural_parameters(writer, &block.structural_parameters)?;
    writer.len("operations", block.operations.len())?;
    for operation in &block.operations {
        encode_operation(writer, operation)?;
    }
    terminator_wire::encode_terminator(writer, &block.terminator)
}

/// One operation row: its id, static reach binding and result, then its
/// kind behind the kind's tag; each kind's layout is owned by its family.
/// A trailing possibly-suspending crossing marker closes the row.
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
        OperationKind::EstablishPlacedReference { source } => {
            storage_operations::encode_establish_placed_reference(writer, source)?
        }
        OperationKind::EstablishReference { source, storage } => {
            storage_operations::encode_establish_reference(writer, source, storage)?
        }
        OperationKind::ReleaseReference { source, path } => {
            storage_operations::encode_release_reference(writer, source, path)?
        }
        OperationKind::EstablishPrimitiveLocal {
            value,
            range_obligation,
        } => value_operations::encode_establish_primitive_local(writer, value, range_obligation)?,
        OperationKind::PrimitiveScalarRead { source, path } => {
            storage_operations::encode_primitive_scalar_read(writer, source, path)?
        }
        OperationKind::StructuralCaseMembership { source, path, case } => {
            storage_operations::encode_structural_case_membership(writer, source, path, case)?
        }
        OperationKind::StructuralLeafCopy { source, path } => {
            storage_operations::encode_structural_leaf_copy(writer, source, path)?
        }
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            storage_operations::encode_structural_case_leaf_copy(writer, source, &path)?
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
        OperationKind::ElementViewWrite {
            destination,
            path,
            index,
            value,
            length,
            obligation,
        } => storage_operations::encode_element_view_write(
            writer,
            destination,
            path,
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
            range_obligation,
        } => storage_operations::encode_write_only_primitive_store(
            writer,
            destination,
            value,
            path,
            range_obligation,
        )?,
        OperationKind::ByteSequenceStore {
            destination,
            source,
            length,
            obligation,
        } => {
            writer.u8(operation_tags::BYTE_SEQUENCE_STORE);
            writer.id(destination);
            writer.id(source);
            writer.id(length);
            writer.id(obligation);
        }
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
        OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            predicate_obligation,
        } => storage_operations::encode_structural_byte_view_field_store(
            writer,
            destination,
            path,
            field,
            source,
            predicate_obligation,
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
        OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            length,
            obligation,
        } => storage_operations::encode_structural_byte_sequence_field_read(
            writer, source, path, field, index, length, obligation,
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
            disposition,
            destination,
            path,
            field,
            value,
        } => storage_operations::encode_store_structural_field(
            writer,
            destination,
            disposition,
            path,
            field,
            value,
        )?,
        OperationKind::EstablishStructuralArray { elements } => {
            value_operations::encode_establish_structural_array(writer, elements)?
        }
        OperationKind::EstablishScalarArray { elements } => {
            value_operations::encode_establish_scalar_array(writer, elements)?
        }
        OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } => value_operations::encode_establish_scalar_case(writer, result_case, fields)?,
        OperationKind::EstablishByteSequenceConcatenation {
            left,
            right,
            capacity_obligation,
        } => value_operations::encode_establish_byte_sequence_concatenation(
            writer,
            left,
            right,
            capacity_obligation,
        )?,
        OperationKind::ByteSequencePredicate { operand, predicate } => {
            value_operations::encode_byte_sequence_predicate(writer, operand, predicate)?
        }
        OperationKind::ByteSequenceEqual { left, right } => {
            value_operations::encode_byte_sequence_equal(writer, left, right)?
        }
        OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes,
            qualifications,
        } => value_operations::encode_establish_byte_sequence_literal(
            writer,
            destination,
            bytes,
            &qualifications,
        )?,
        OperationKind::EstablishElementView {
            region,
            destination,
            source,
            element,
        } => storage_operations::encode_establish_element_view(
            writer,
            destination,
            source,
            element,
            region,
        )?,
        OperationKind::EstablishMutableElementView {
            region,
            destination,
            source,
            element,
        } => storage_operations::encode_establish_mutable_element_view(
            writer,
            destination,
            source,
            element,
            region,
        )?,
        OperationKind::ElementViewLength { source } => {
            storage_operations::encode_element_view_length(writer, source)?
        }
        OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        } => storage_operations::encode_element_view_read(
            writer, source, index, length, obligation, path,
        )?,
        OperationKind::ElementViewSubslice {
            source,
            start,
            end,
            length,
            obligation,
        } => storage_operations::encode_element_view_subslice(
            writer, source, start, end, length, obligation,
        )?,
        OperationKind::EstablishZeroedStructural {} => {
            value_operations::encode_establish_zeroed_structural(writer)?
        }
        OperationKind::EstablishStructuralLocal {} => {
            value_operations::encode_establish_structural_local(writer)?
        }
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            value_operations::encode_establish_trivial_affine_local(writer, destination)?
        }
        OperationKind::EstablishRecord { fields } => {
            value_operations::encode_establish_record(writer, fields)?
        }
        OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        } => value_operations::encode_establish_structural_case(writer, result_case, fields)?,
        OperationKind::Call {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call(
            writer,
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments.clone(),
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::SelectImplementation { table } => {
            call_operations::encode_select_implementation(writer, table)
        }
        OperationKind::CallDynamic {
            selector,
            interface,
            slot,
            arguments,
            family_actuals,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_dynamic(
            writer,
            selector,
            interface,
            slot,
            arguments,
            family_actuals,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallUnit {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_unit(
            writer,
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments.clone(),
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallStructuralScalar {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        } => call_operations::encode_call_structural_scalar(
            writer,
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments.clone(),
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        )?,
        OperationKind::CallStructural {
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => call_operations::encode_call_structural(
            writer,
            callee,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        )?,
        OperationKind::CallStructuralWithScalarArguments {
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        } => call_operations::encode_call_structural_with_scalar_arguments(
            writer,
            callee,
            arguments,
            erased_arguments,
            erased_proof_arguments.clone(),
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
            selected_evidence,
        )?,
        OperationKind::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
        } => call_operations::encode_boundary_call(
            writer,
            boundary,
            arguments,
            structural_arguments,
            completion_receipts,
            requirement_obligations,
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
        OperationKind::ScalarRepresentationView {
            operand,
            landing_obligation,
        } => scalar_operations::encode_scalar_representation_view(
            writer,
            operand,
            landing_obligation,
        )?,
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
        OperationKind::ScalarStructuralField {
            source,
            ref path,
            field,
        } => storage_operations::encode_scalar_structural_field(writer, source, path, field)?,
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
        OperationKind::TrappingInteger { operation } => {
            scalar_operations::encode_trapping_integer(writer, operation)?
        }
        OperationKind::FloatResultPolicy { policy } => {
            scalar_operations::encode_float_result_policy(writer, &policy)?;
        }
        OperationKind::AtomicFence { ordering } => {
            atomic_operations::encode_atomic_fence(writer, ordering);
        }
        OperationKind::AtomicAccess {
            place,
            path,
            field,
            event,
        } => atomic_operations::encode_atomic_access(writer, place, &path, field, event)?,
    }
    writer.boolean(operation.suspension_crossing.is_some());
    if let Some(crossing) = operation.suspension_crossing {
        writer.id(crossing);
    }
    Ok(())
}

pub(crate) fn decode_block(reader: &mut Reader<'_>) -> Result<Block, CodecError> {
    let id = reader.id("BlockId")?;
    let parameters = decode_declarations(reader)?;
    let erased_scalar_formals = decode_declarations(reader)?;
    let erased_proof_formals = decode_proof_formals(reader)?;
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
        erased_scalar_formals,
        erased_proof_formals,
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
        operation_tags::ESTABLISH_ELEMENT_VIEW => {
            storage_operations::decode_establish_element_view(reader)?
        }
        operation_tags::ESTABLISH_MUTABLE_ELEMENT_VIEW => {
            storage_operations::decode_establish_mutable_element_view(reader)?
        }
        operation_tags::ELEMENT_VIEW_LENGTH => {
            storage_operations::decode_element_view_length(reader)?
        }
        operation_tags::ELEMENT_VIEW_READ => storage_operations::decode_element_view_read(reader)?,
        operation_tags::PROJECTED_ELEMENT_VIEW_READ => {
            storage_operations::decode_projected_element_view_read(reader)?
        }
        operation_tags::ELEMENT_VIEW_SUBSLICE => {
            storage_operations::decode_element_view_subslice(reader)?
        }
        operation_tags::BYTE_SEQUENCE_WRITE => {
            storage_operations::decode_byte_sequence_write(reader)?
        }
        operation_tags::ELEMENT_VIEW_WRITE => {
            storage_operations::decode_element_view_write(reader)?
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
            storage_operations::decode_establish_reference(reader, false)?
        }
        operation_tags::ESTABLISH_PLACED_REFERENCE => {
            storage_operations::decode_establish_placed_reference(reader)?
        }
        operation_tags::ESTABLISH_STORAGE_VIEW_REFERENCE => {
            storage_operations::decode_establish_reference(reader, true)?
        }
        operation_tags::RELEASE_REFERENCE => {
            storage_operations::decode_release_reference(reader, false)?
        }
        operation_tags::PROJECTED_RELEASE_REFERENCE => {
            storage_operations::decode_release_reference(reader, true)?
        }
        operation_tags::PRIMITIVE_SCALAR_READ => {
            storage_operations::decode_primitive_scalar_read(reader)?
        }
        operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ => {
            storage_operations::decode_projected_primitive_scalar_read(reader)?
        }
        operation_tags::RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_restricted_write_only_primitive_store(reader)?
        }
        operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_projected_write_only_primitive_store(reader)?
        }
        operation_tags::STRUCTURAL_CASE_MEMBERSHIP => {
            storage_operations::decode_structural_case_membership(reader)?
        }
        operation_tags::STRUCTURAL_LEAF_COPY => {
            storage_operations::decode_structural_leaf_copy(reader)?
        }
        operation_tags::STRUCTURAL_CASE_LEAF_COPY => {
            storage_operations::decode_structural_case_leaf_copy(reader)?
        }
        operation_tags::WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_write_only_primitive_store(reader)?
        }
        operation_tags::BYTE_SEQUENCE_STORE => OperationKind::ByteSequenceStore {
            destination: reader.id("PlaceId")?,
            source: reader.id("PlaceId")?,
            length: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
        },
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE => {
            storage_operations::decode_structural_byte_sequence_field_store(reader)?
        }
        operation_tags::STRUCTURAL_BYTE_VIEW_FIELD_STORE => {
            storage_operations::decode_structural_byte_view_field_store(reader, false)?
        }
        operation_tags::PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE => {
            storage_operations::decode_structural_byte_view_field_store(reader, true)?
        }
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH => {
            storage_operations::decode_structural_byte_sequence_field_length(reader)?
        }
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE => {
            storage_operations::decode_structural_byte_sequence_field_byte_store(reader)?
        }
        operation_tags::STRUCTURAL_BYTE_SEQUENCE_FIELD_READ => {
            storage_operations::decode_structural_byte_sequence_field_read(reader)?
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
            storage_operations::decode_store_structural_field(
                reader,
                terminal_psi::StructuralStoreDisposition::Vacant,
            )?
        }
        operation_tags::REPLACE_DISCARDABLE_STRUCTURAL_FIELD => {
            storage_operations::decode_store_structural_field(
                reader,
                terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
            )?
        }
        operation_tags::ESTABLISH_SCALAR_ARRAY => {
            value_operations::decode_establish_scalar_array(reader)?
        }
        operation_tags::ESTABLISH_SCALAR_CASE => {
            value_operations::decode_establish_scalar_case(reader)?
        }
        operation_tags::ESTABLISH_BYTE_SEQUENCE_CONCATENATION => {
            value_operations::decode_establish_byte_sequence_concatenation(reader)?
        }
        operation_tags::BYTE_SEQUENCE_PREDICATE => {
            value_operations::decode_byte_sequence_predicate(reader)?
        }
        operation_tags::BYTE_SEQUENCE_EQUAL => {
            value_operations::decode_byte_sequence_equal(reader)?
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
        operation_tags::SCALAR_REPRESENTATION_VIEW => {
            scalar_operations::decode_scalar_representation_view(reader)?
        }
        operation_tags::LANDED_SCALAR_REPRESENTATION_VIEW => {
            scalar_operations::decode_landed_scalar_representation_view(reader)?
        }
        operation_tags::NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD => {
            scalar_operations::decode_nearest_ieee_float_fused_multiply_add(reader)?
        }
        operation_tags::BOOLEAN_STRUCTURAL_FIELD => {
            storage_operations::decode_boolean_structural_field(reader)?
        }
        operation_tags::SCALAR_STRUCTURAL_FIELD => {
            storage_operations::decode_scalar_structural_field(reader)?
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
        operation_tags::SELECT_IMPLEMENTATION => {
            call_operations::decode_select_implementation(reader)?
        }
        operation_tags::CALL_DYNAMIC => call_operations::decode_call_dynamic(reader)?,
        operation_tags::BOUNDARY_CALL => call_operations::decode_boundary_call(reader)?,
        operation_tags::PORT_WRITE => call_operations::decode_port_write(reader)?,
        operation_tags::ESTABLISH_ZEROED_STRUCTURAL => {
            value_operations::decode_establish_zeroed_structural(reader)?
        }
        operation_tags::ESTABLISH_STRUCTURAL_LOCAL => {
            value_operations::decode_establish_structural_local(reader)?
        }
        operation_tags::ESTABLISH_TRIVIAL_AFFINE_LOCAL => {
            value_operations::decode_establish_trivial_affine_local(reader)?
        }
        operation_tags::ESTABLISH_STRUCTURAL_ARRAY => {
            value_operations::decode_establish_structural_array(reader)?
        }
        operation_tags::ESTABLISH_RECORD => value_operations::decode_establish_record(reader)?,
        operation_tags::ESTABLISH_STRUCTURAL_CASE => {
            value_operations::decode_establish_structural_case(reader)?
        }
        operation_tags::CALL_STRUCTURAL_SCALAR => {
            call_operations::decode_call_structural_scalar(reader)?
        }
        operation_tags::CALL_STRUCTURAL => call_operations::decode_call_structural(reader)?,
        operation_tags::CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS => {
            call_operations::decode_call_structural_with_scalar_arguments(reader)?
        }
        operation_tags::TRAPPING_INTEGER => scalar_operations::decode_trapping_integer(reader)?,
        operation_tags::FLOAT_RESULT_POLICY => {
            scalar_operations::decode_float_result_policy(reader)?
        }
        operation_tags::ATOMIC_FENCE => atomic_operations::decode_atomic_fence(reader)?,
        operation_tags::ATOMIC_ACCESS => atomic_operations::decode_atomic_access(reader)?,
        tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
    };
    let suspension_crossing = if reader.boolean()? {
        Some(reader.id("SuspensionCrossingId")?)
    } else {
        None
    };
    Ok(Operation {
        static_reach_binding,
        suspension_crossing,
        id: operation_id,
        result,
        kind,
    })
}
