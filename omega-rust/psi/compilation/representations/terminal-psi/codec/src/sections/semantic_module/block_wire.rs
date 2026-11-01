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

#[cfg(test)]
mod owned_value_tests;

use semantic_vocabulary::CanonicalStructuralPathSegment;
use terminal_psi::{Block, Operation, OperationKind, OperationResult};

use self::atomic_operations::{
    decode_atomic_access, decode_atomic_fence, encode_atomic_access, encode_atomic_fence,
};
use self::operation_tags::{
    ATOMIC_ACCESS, ATOMIC_FENCE, BOOLEAN_CONSTANT, BOOLEAN_EQUAL, BOOLEAN_NOT,
    BOOLEAN_STRUCTURAL_FIELD, BOUNDARY_CALL, BYTE_SEQUENCE_EQUAL, BYTE_SEQUENCE_LENGTH,
    BYTE_SEQUENCE_PREDICATE, BYTE_SEQUENCE_READ, BYTE_SEQUENCE_STORE, BYTE_SEQUENCE_SUBSLICE,
    BYTE_SEQUENCE_WRITE, CALL, CALL_DYNAMIC, CALL_STRUCTURAL, CALL_STRUCTURAL_SCALAR,
    CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS, CALL_UNIT, ELEMENT_VIEW_LENGTH, ELEMENT_VIEW_READ,
    ELEMENT_VIEW_SUBSLICE, ELEMENT_VIEW_WRITE, ESTABLISH_ACCESS_REFERENCE,
    ESTABLISH_ACTIVATION_CLAIM, ESTABLISH_BYTE_SEQUENCE_CONCATENATION,
    ESTABLISH_BYTE_SEQUENCE_LITERAL, ESTABLISH_ELEMENT_VIEW, ESTABLISH_MUTABLE_ELEMENT_VIEW,
    ESTABLISH_NATIVE_CODE_LAYOUT, ESTABLISH_PRIMITIVE_LOCAL, ESTABLISH_RECORD, ESTABLISH_REFERENCE,
    ESTABLISH_SCALAR_ARRAY, ESTABLISH_SCALAR_CASE, ESTABLISH_STORAGE_VIEW_REFERENCE,
    ESTABLISH_STRUCTURAL_ARRAY, ESTABLISH_STRUCTURAL_CASE, ESTABLISH_STRUCTURAL_LOCAL,
    ESTABLISH_TRIVIAL_AFFINE_LOCAL, ESTABLISH_ZEROED_STRUCTURAL, EXACT_INTEGER_ADD,
    EXACT_INTEGER_DIVIDE, EXACT_INTEGER_MULTIPLY, EXACT_INTEGER_REMAINDER,
    EXACT_INTEGER_SHIFT_LEFT, EXACT_INTEGER_SHIFT_RIGHT, EXACT_INTEGER_SUBTRACT,
    FLOAT_RESULT_POLICY, IEEE_FLOAT_COMPARE, IEEE_FLOAT_CONSTANT, INTEGER_BITWISE_AND,
    INTEGER_BITWISE_NOT, INTEGER_BITWISE_OR, INTEGER_BITWISE_XOR, INTEGER_CONSTANT, INTEGER_EQUAL,
    INTEGER_EXACT_CAST, INTEGER_LESS_OR_EQUAL, INTEGER_LESS_THAN, INTEGER_WIDEN,
    LANDED_SCALAR_REPRESENTATION_VIEW, MERGE_ACTIVATION_CLAIM, MOVE_STRUCTURAL_FIELD,
    MOVE_STRUCTURAL_VALUE, NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD, PORT_WRITE,
    PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE, PRIMITIVE_SCALAR_READ,
    PROJECTED_ELEMENT_VIEW_READ, PROJECTED_PRIMITIVE_SCALAR_READ, PROJECTED_RELEASE_REFERENCE,
    PROJECTED_WRITE_ONLY_PRIMITIVE_STORE, RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE,
    READ_SCALAR_APPLICATION, REGISTER_ACCESS, REGISTER_OBSERVATION, RELEASE_ACTIVATION_CLAIM,
    RELEASE_REFERENCE, REPLACE_DISCARDABLE_STRUCTURAL_FIELD, RESTRICTED_ATOMIC_ACCESS,
    RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE, SATURATING_INTEGER_ADD, SATURATING_INTEGER_DIVIDE,
    SATURATING_INTEGER_MULTIPLY, SATURATING_INTEGER_REMAINDER, SATURATING_INTEGER_SUBTRACT,
    SCALAR_REPRESENTATION_VIEW, SCALAR_STRUCTURAL_FIELD, SELECT_IMPLEMENTATION,
    SPLIT_ACTIVATION_CLAIM, STORAGE_ADDRESS, STORE_STRUCTURAL_FIELD,
    STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE, STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH,
    STRUCTURAL_BYTE_SEQUENCE_FIELD_READ, STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE,
    STRUCTURAL_BYTE_VIEW_FIELD_STORE, STRUCTURAL_CASE_LEAF_COPY, STRUCTURAL_CASE_MEMBERSHIP,
    STRUCTURAL_LEAF_COPY, STRUCTURAL_SCALAR_FIELD_STORE, TRANSFORM_CONTENT, TRAPPING_INTEGER,
    WRAPPING_INTEGER_ADD, WRAPPING_INTEGER_DIVIDE, WRAPPING_INTEGER_MULTIPLY,
    WRAPPING_INTEGER_REMAINDER, WRAPPING_INTEGER_SHIFT_LEFT, WRAPPING_INTEGER_SHIFT_RIGHT,
    WRAPPING_INTEGER_SUBTRACT, WRITE_ONLY_PRIMITIVE_STORE,
};
use self::scalar_operations::{
    decode_boolean_equal, decode_boolean_not, decode_exact_integer_add,
    decode_exact_integer_divide, decode_exact_integer_multiply, decode_exact_integer_remainder,
    decode_exact_integer_shift_left, decode_exact_integer_shift_right,
    decode_exact_integer_subtract, decode_float_result_policy, decode_ieee_float_compare,
    decode_integer_bitwise_and, decode_integer_bitwise_not, decode_integer_bitwise_or,
    decode_integer_bitwise_xor, decode_integer_equal, decode_integer_exact_cast,
    decode_integer_less_or_equal, decode_integer_less_than, decode_integer_widen,
    decode_landed_scalar_representation_view, decode_nearest_ieee_float_fused_multiply_add,
    decode_saturating_integer_add, decode_saturating_integer_divide,
    decode_saturating_integer_multiply, decode_saturating_integer_remainder,
    decode_saturating_integer_subtract, decode_scalar_representation_view, decode_trapping_integer,
    decode_wrapping_integer_add, decode_wrapping_integer_divide, decode_wrapping_integer_multiply,
    decode_wrapping_integer_remainder, decode_wrapping_integer_shift_left,
    decode_wrapping_integer_shift_right, decode_wrapping_integer_subtract, encode_boolean_equal,
    encode_boolean_not, encode_exact_integer_add, encode_exact_integer_divide,
    encode_exact_integer_multiply, encode_exact_integer_remainder, encode_exact_integer_shift_left,
    encode_exact_integer_shift_right, encode_exact_integer_subtract, encode_float_result_policy,
    encode_ieee_float_compare, encode_integer_bitwise_and, encode_integer_bitwise_not,
    encode_integer_bitwise_or, encode_integer_bitwise_xor, encode_integer_equal,
    encode_integer_exact_cast, encode_integer_less_or_equal, encode_integer_less_than,
    encode_integer_widen, encode_nearest_ieee_float_fused_multiply_add,
    encode_saturating_integer_add, encode_saturating_integer_divide,
    encode_saturating_integer_multiply, encode_saturating_integer_remainder,
    encode_saturating_integer_subtract, encode_scalar_representation_view, encode_trapping_integer,
    encode_wrapping_integer_add, encode_wrapping_integer_divide, encode_wrapping_integer_multiply,
    encode_wrapping_integer_remainder, encode_wrapping_integer_shift_left,
    encode_wrapping_integer_shift_right, encode_wrapping_integer_subtract,
};
use self::storage_operations::{
    decode_boolean_structural_field, decode_byte_sequence_length, decode_byte_sequence_read,
    decode_byte_sequence_subslice, decode_byte_sequence_write, decode_element_view_length,
    decode_element_view_read, decode_element_view_subslice, decode_element_view_write,
    decode_establish_access_reference, decode_establish_element_view,
    decode_establish_mutable_element_view, decode_establish_reference,
    decode_move_structural_field, decode_move_structural_value, decode_primitive_scalar_read,
    decode_projected_element_view_read, decode_projected_primitive_scalar_read,
    decode_projected_write_only_primitive_store,
    decode_range_checked_structural_scalar_field_store, decode_release_reference,
    decode_restricted_write_only_primitive_store, decode_scalar_structural_field,
    decode_store_structural_field, decode_structural_byte_sequence_field_byte_store,
    decode_structural_byte_sequence_field_length, decode_structural_byte_sequence_field_read,
    decode_structural_byte_sequence_field_store, decode_structural_byte_view_field_store,
    decode_structural_case_leaf_copy, decode_structural_case_membership,
    decode_structural_leaf_copy, decode_structural_scalar_field_store,
    decode_write_only_primitive_store, encode_boolean_structural_field,
    encode_byte_sequence_length, encode_byte_sequence_read, encode_byte_sequence_subslice,
    encode_byte_sequence_write, encode_element_view_length, encode_element_view_read,
    encode_element_view_subslice, encode_element_view_write, encode_establish_access_reference,
    encode_establish_element_view, encode_establish_mutable_element_view,
    encode_establish_reference, encode_move_structural_field, encode_move_structural_value,
    encode_primitive_scalar_read, encode_release_reference, encode_scalar_structural_field,
    encode_store_structural_field, encode_structural_byte_sequence_field_byte_store,
    encode_structural_byte_sequence_field_length, encode_structural_byte_sequence_field_read,
    encode_structural_byte_sequence_field_store, encode_structural_byte_view_field_store,
    encode_structural_case_leaf_copy, encode_structural_case_membership,
    encode_structural_leaf_copy, encode_structural_scalar_field_store,
    encode_write_only_primitive_store,
};
use self::value_operations::{
    decode_boolean_constant, decode_byte_sequence_equal, decode_byte_sequence_predicate,
    decode_establish_byte_sequence_concatenation, decode_establish_byte_sequence_literal,
    decode_establish_primitive_local, decode_establish_record, decode_establish_scalar_array,
    decode_establish_scalar_case, decode_establish_structural_array,
    decode_establish_structural_case, decode_establish_structural_local,
    decode_establish_trivial_affine_local, decode_establish_zeroed_structural,
    decode_ieee_float_constant, decode_integer_constant, encode_boolean_constant,
    encode_byte_sequence_equal, encode_byte_sequence_predicate,
    encode_establish_byte_sequence_concatenation, encode_establish_byte_sequence_literal,
    encode_establish_primitive_local, encode_establish_record, encode_establish_scalar_array,
    encode_establish_scalar_case, encode_establish_structural_array,
    encode_establish_structural_case, encode_establish_structural_local,
    encode_establish_trivial_affine_local, encode_establish_zeroed_structural,
    encode_ieee_float_constant, encode_integer_constant,
};
use super::CodecError;
use super::machine_wire::{
    decode_declaration, decode_declarations, encode_declaration, encode_declarations,
};
use super::proof_term_wire::{decode_proof_formals, encode_proof_formals};
use super::structural_field_wire::encode_canonical_structural_path;
use super::structural_place_wire::{
    decode_structural_arguments, decode_structural_path, encode_structural_arguments,
    encode_structural_path,
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
        4 => Ok(CanonicalStructuralPathSegment::Referent),
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
    writer.counted("operations", &block.operations, encode_operation)?;
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
        OperationKind::ReadScalarApplication { application } => {
            writer.u8(READ_SCALAR_APPLICATION);
            writer.id(application);
        }
        OperationKind::StorageAddress { source } => {
            writer.u8(STORAGE_ADDRESS);
            writer.id(source.place);
            super::structural_signature_wire::encode_structural_access(writer, source.access);
            encode_structural_path(writer, "storage address source path", &source.path)?;
        }
        OperationKind::EstablishAccessReference { source, resident } => {
            encode_establish_access_reference(writer, source, resident)?
        }
        OperationKind::EstablishReference { source, storage } => {
            encode_establish_reference(writer, source, storage)?
        }
        OperationKind::ReleaseReference { source, path } => {
            encode_release_reference(writer, source, path)?
        }
        OperationKind::EstablishPrimitiveLocal {
            value,
            range_obligation,
        } => encode_establish_primitive_local(writer, value, range_obligation)?,
        OperationKind::PrimitiveScalarRead { source, path } => {
            encode_primitive_scalar_read(writer, source, path)?
        }
        OperationKind::StructuralCaseMembership { source, path, case } => {
            encode_structural_case_membership(writer, source, path, case)?
        }
        OperationKind::StructuralLeafCopy { source, path } => {
            encode_structural_leaf_copy(writer, source, path)?
        }
        OperationKind::MoveStructuralValue { source } => {
            encode_move_structural_value(writer, source)?
        }
        OperationKind::TransformContent {
            source,
            parents,
            derivations,
        } => {
            writer.u8(TRANSFORM_CONTENT);
            encode_structural_arguments(writer, &[source])?;
            encode_structural_arguments(writer, &parents)?;
            super::content_wire::encode_content_transformations(writer, &derivations)?;
        }
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            encode_structural_case_leaf_copy(writer, source, &path)?
        }
        OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            obligation,
        } => encode_byte_sequence_subslice(writer, source, start, end, length, obligation)?,
        OperationKind::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            obligation,
        } => encode_byte_sequence_write(writer, destination, index, value, length, obligation)?,
        OperationKind::ElementViewWrite {
            destination,
            path,
            index,
            value,
            length,
            obligation,
        } => {
            encode_element_view_write(writer, destination, path, index, value, length, obligation)?
        }
        OperationKind::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
        } => encode_byte_sequence_read(writer, source, index, length, obligation)?,
        OperationKind::ByteSequenceLength { source } => {
            encode_byte_sequence_length(writer, source)?
        }
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            path,
            range_obligation,
        } => encode_write_only_primitive_store(writer, destination, value, path, range_obligation)?,
        OperationKind::ByteSequenceStore {
            destination,
            source,
            length,
            obligation,
        } => {
            writer.u8(BYTE_SEQUENCE_STORE);
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
        } => encode_structural_byte_sequence_field_store(
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
        } => encode_structural_byte_view_field_store(
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
        } => encode_structural_byte_sequence_field_length(writer, source, path, field)?,
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            index,
            value,
            length,
            obligation,
        } => encode_structural_byte_sequence_field_byte_store(
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
        } => encode_structural_byte_sequence_field_read(
            writer, source, path, field, index, length, obligation,
        )?,
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
            range_obligation,
        } => encode_structural_scalar_field_store(
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
        } => encode_move_structural_field(writer, source, path, field)?,
        OperationKind::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
        } => encode_store_structural_field(writer, destination, disposition, path, field, value)?,
        OperationKind::EstablishStructuralArray { elements } => {
            encode_establish_structural_array(writer, elements)?
        }
        OperationKind::EstablishActivationClaim {
            bound,
            committed,
            backing_type,
            established_case,
            rejected_case,
        } => {
            writer.u8(ESTABLISH_ACTIVATION_CLAIM);
            writer.u64(bound);
            writer.id(committed);
            writer.id(backing_type);
            writer.id(established_case);
            writer.id(rejected_case);
        }
        OperationKind::SplitActivationClaim {
            source,
            claim,
            bound_at,
            committed_at,
            covered_case,
            rejected_case,
        } => {
            writer.u8(SPLIT_ACTIVATION_CLAIM);
            writer.id(source.place);
            super::structural_signature_wire::encode_structural_access(writer, source.access);
            encode_structural_path(writer, "activation claim split", &source.path)?;
            writer.id(claim);
            writer.u64(bound_at);
            writer.id(committed_at);
            writer.id(covered_case);
            writer.id(rejected_case);
        }
        OperationKind::MergeActivationClaim {
            lower,
            upper,
            lower_claim,
            upper_claim,
            covered_case,
            rejected_case,
        } => {
            writer.u8(MERGE_ACTIVATION_CLAIM);
            for (argument, label) in [
                (lower, "activation claim merge lower"),
                (upper, "activation claim merge upper"),
            ] {
                writer.id(argument.place);
                super::structural_signature_wire::encode_structural_access(writer, argument.access);
                encode_structural_path(writer, label, &argument.path)?;
            }
            writer.id(lower_claim);
            writer.id(upper_claim);
            writer.id(covered_case);
            writer.id(rejected_case);
        }
        OperationKind::ReleaseActivationClaim { source, claim } => {
            writer.u8(RELEASE_ACTIVATION_CLAIM);
            writer.id(source.place);
            super::structural_signature_wire::encode_structural_access(writer, source.access);
            encode_structural_path(writer, "activation claim release", &source.path)?;
            writer.id(claim);
        }
        OperationKind::EstablishScalarArray { elements } => {
            encode_establish_scalar_array(writer, elements)?
        }
        OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } => encode_establish_scalar_case(writer, result_case, fields)?,
        OperationKind::EstablishByteSequenceConcatenation {
            left,
            right,
            capacity_obligation,
        } => {
            encode_establish_byte_sequence_concatenation(writer, left, right, capacity_obligation)?
        }
        OperationKind::ByteSequencePredicate { operand, predicate } => {
            encode_byte_sequence_predicate(writer, operand, predicate)?
        }
        OperationKind::ByteSequenceEqual { left, right } => {
            encode_byte_sequence_equal(writer, left, right)?
        }
        OperationKind::EstablishByteSequenceLiteral {
            destination,
            bytes,
            qualifications,
        } => encode_establish_byte_sequence_literal(writer, destination, bytes, &qualifications)?,
        OperationKind::EstablishElementView {
            region,
            destination,
            source,
            element,
        } => encode_establish_element_view(writer, destination, source, element, region)?,
        OperationKind::EstablishMutableElementView {
            region,
            destination,
            source,
            element,
        } => encode_establish_mutable_element_view(writer, destination, source, element, region)?,
        OperationKind::ElementViewLength { source } => encode_element_view_length(writer, source)?,
        OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        } => encode_element_view_read(writer, source, index, length, obligation, path)?,
        OperationKind::ElementViewSubslice {
            source,
            start,
            end,
            length,
            obligation,
        } => encode_element_view_subslice(writer, source, start, end, length, obligation)?,
        OperationKind::EstablishZeroedStructural {} => encode_establish_zeroed_structural(writer)?,
        OperationKind::EstablishStructuralLocal {} => encode_establish_structural_local(writer)?,
        OperationKind::EstablishTrivialAffineLocal { destination } => {
            encode_establish_trivial_affine_local(writer, destination)?
        }
        OperationKind::EstablishNativeCodeLayout { machine } => {
            writer.u8(ESTABLISH_NATIVE_CODE_LAYOUT);
            writer.u64(machine.get());
        }
        OperationKind::EstablishRecord { fields } => encode_establish_record(writer, fields)?,
        OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        } => encode_establish_structural_case(writer, result_case, fields)?,
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
        } => call_operations::encode_tagged_module_invocation(
            writer,
            CALL_UNIT,
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
        } => call_operations::encode_tagged_module_invocation(
            writer,
            CALL_STRUCTURAL_SCALAR,
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
        OperationKind::RegisterAccess { service, event } => {
            call_operations::encode_register_access(writer, service, event)?
        }
        OperationKind::IntegerConstant { value } => encode_integer_constant(writer, value)?,
        OperationKind::BooleanConstant { value } => encode_boolean_constant(writer, value)?,
        OperationKind::IeeeFloatConstant { value } => encode_ieee_float_constant(writer, value)?,
        OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        } => encode_ieee_float_compare(writer, comparison, left, right)?,
        OperationKind::ScalarRepresentationView {
            operand,
            landing_obligation,
        } => encode_scalar_representation_view(writer, operand, landing_obligation)?,
        OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } => encode_nearest_ieee_float_fused_multiply_add(writer, left, right, addend)?,
        OperationKind::BooleanStructuralField {
            source,
            ref path,
            field,
        } => encode_boolean_structural_field(writer, source, path, field)?,
        OperationKind::ScalarStructuralField {
            source,
            ref path,
            field,
        } => encode_scalar_structural_field(writer, source, path, field)?,
        OperationKind::BooleanNot { operand } => encode_boolean_not(writer, operand)?,
        OperationKind::BooleanEqual { left, right } => encode_boolean_equal(writer, left, right)?,
        OperationKind::IntegerEqual { left, right } => encode_integer_equal(writer, left, right)?,
        OperationKind::IntegerLessThan { left, right } => {
            encode_integer_less_than(writer, left, right)?
        }
        OperationKind::IntegerLessOrEqual { left, right } => {
            encode_integer_less_or_equal(writer, left, right)?
        }
        OperationKind::IntegerBitwiseNot { operand } => {
            encode_integer_bitwise_not(writer, operand)?
        }
        OperationKind::IntegerWiden { operand } => encode_integer_widen(writer, operand)?,
        OperationKind::IntegerExactCast {
            operand,
            obligation,
        } => encode_integer_exact_cast(writer, operand, obligation)?,
        OperationKind::IntegerBitwiseAnd { left, right } => {
            encode_integer_bitwise_and(writer, left, right)?
        }
        OperationKind::IntegerBitwiseOr { left, right } => {
            encode_integer_bitwise_or(writer, left, right)?
        }
        OperationKind::IntegerBitwiseXor { left, right } => {
            encode_integer_bitwise_xor(writer, left, right)?
        }
        OperationKind::WrappingIntegerShiftLeft { value, count } => {
            encode_wrapping_integer_shift_left(writer, value, count)?
        }
        OperationKind::WrappingIntegerShiftRight { value, count } => {
            encode_wrapping_integer_shift_right(writer, value, count)?
        }
        OperationKind::ExactIntegerShiftLeft {
            value,
            count,
            obligation,
        } => encode_exact_integer_shift_left(writer, value, count, obligation)?,
        OperationKind::ExactIntegerShiftRight {
            value,
            count,
            obligation,
        } => encode_exact_integer_shift_right(writer, value, count, obligation)?,
        OperationKind::ExactIntegerAdd {
            left,
            right,
            obligation,
        } => encode_exact_integer_add(writer, left, right, obligation)?,
        OperationKind::ExactIntegerSubtract {
            left,
            right,
            obligation,
        } => encode_exact_integer_subtract(writer, left, right, obligation)?,
        OperationKind::ExactIntegerMultiply {
            left,
            right,
            obligation,
        } => encode_exact_integer_multiply(writer, left, right, obligation)?,
        OperationKind::ExactIntegerDivide {
            left,
            right,
            obligation,
        } => encode_exact_integer_divide(writer, left, right, obligation)?,
        OperationKind::ExactIntegerRemainder {
            left,
            right,
            obligation,
        } => encode_exact_integer_remainder(writer, left, right, obligation)?,
        OperationKind::WrappingIntegerDivide {
            left,
            right,
            obligation,
        } => encode_wrapping_integer_divide(writer, left, right, obligation)?,
        OperationKind::WrappingIntegerRemainder {
            left,
            right,
            obligation,
        } => encode_wrapping_integer_remainder(writer, left, right, obligation)?,
        OperationKind::SaturatingIntegerDivide {
            left,
            right,
            obligation,
        } => encode_saturating_integer_divide(writer, left, right, obligation)?,
        OperationKind::SaturatingIntegerRemainder {
            left,
            right,
            obligation,
        } => encode_saturating_integer_remainder(writer, left, right, obligation)?,
        OperationKind::WrappingIntegerAdd { left, right } => {
            encode_wrapping_integer_add(writer, left, right)?
        }
        OperationKind::SaturatingIntegerAdd { left, right } => {
            encode_saturating_integer_add(writer, left, right)?
        }
        OperationKind::WrappingIntegerSubtract { left, right } => {
            encode_wrapping_integer_subtract(writer, left, right)?
        }
        OperationKind::SaturatingIntegerSubtract { left, right } => {
            encode_saturating_integer_subtract(writer, left, right)?
        }
        OperationKind::WrappingIntegerMultiply { left, right } => {
            encode_wrapping_integer_multiply(writer, left, right)?
        }
        OperationKind::SaturatingIntegerMultiply { left, right } => {
            encode_saturating_integer_multiply(writer, left, right)?
        }
        OperationKind::TrappingInteger { operation } => encode_trapping_integer(writer, operation)?,
        OperationKind::FloatResultPolicy { policy } => {
            encode_float_result_policy(writer, &policy)?;
        }
        OperationKind::AtomicFence { ordering } => {
            encode_atomic_fence(writer, ordering);
        }
        OperationKind::AtomicAccess {
            place,
            path,
            field,
            event,
            resident_obligation,
        } => encode_atomic_access(writer, place, &path, field, event, resident_obligation)?,
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
    let operations = decode_counted(reader, decode_operation)?;
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
        READ_SCALAR_APPLICATION => OperationKind::ReadScalarApplication {
            application: reader.id("ScalarApplicationId")?,
        },
        ESTABLISH_NATIVE_CODE_LAYOUT => OperationKind::EstablishNativeCodeLayout {
            machine: reader.id("native code layout machine")?,
        },
        STORAGE_ADDRESS => OperationKind::StorageAddress {
            source: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
        },
        BYTE_SEQUENCE_SUBSLICE => decode_byte_sequence_subslice(reader)?,
        ESTABLISH_ELEMENT_VIEW => decode_establish_element_view(reader)?,
        ESTABLISH_MUTABLE_ELEMENT_VIEW => decode_establish_mutable_element_view(reader)?,
        ELEMENT_VIEW_LENGTH => decode_element_view_length(reader)?,
        ELEMENT_VIEW_READ => decode_element_view_read(reader)?,
        PROJECTED_ELEMENT_VIEW_READ => decode_projected_element_view_read(reader)?,
        ELEMENT_VIEW_SUBSLICE => decode_element_view_subslice(reader)?,
        BYTE_SEQUENCE_WRITE => decode_byte_sequence_write(reader)?,
        ELEMENT_VIEW_WRITE => decode_element_view_write(reader)?,
        BYTE_SEQUENCE_READ => decode_byte_sequence_read(reader)?,
        BYTE_SEQUENCE_LENGTH => decode_byte_sequence_length(reader)?,
        ESTABLISH_PRIMITIVE_LOCAL => decode_establish_primitive_local(reader)?,
        ESTABLISH_REFERENCE => decode_establish_reference(reader, false)?,
        ESTABLISH_ACCESS_REFERENCE => decode_establish_access_reference(reader)?,
        ESTABLISH_STORAGE_VIEW_REFERENCE => decode_establish_reference(reader, true)?,
        RELEASE_REFERENCE => decode_release_reference(reader, false)?,
        PROJECTED_RELEASE_REFERENCE => decode_release_reference(reader, true)?,
        PRIMITIVE_SCALAR_READ => decode_primitive_scalar_read(reader)?,
        PROJECTED_PRIMITIVE_SCALAR_READ => decode_projected_primitive_scalar_read(reader)?,
        RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE => {
            decode_restricted_write_only_primitive_store(reader)?
        }
        PROJECTED_WRITE_ONLY_PRIMITIVE_STORE => {
            decode_projected_write_only_primitive_store(reader)?
        }
        STRUCTURAL_CASE_MEMBERSHIP => decode_structural_case_membership(reader)?,
        STRUCTURAL_LEAF_COPY => decode_structural_leaf_copy(reader)?,
        TRANSFORM_CONTENT => {
            let mut sources = decode_structural_arguments(reader)?;
            if sources.len() != 1 {
                return Err(CodecError::MalformedStructuralFoundation(
                    "content transformation requires one payload",
                ));
            }
            let source = sources.remove(0);
            OperationKind::TransformContent {
                source,
                parents: decode_structural_arguments(reader)?,
                derivations: super::content_wire::decode_content_transformations(reader)?,
            }
        }
        MOVE_STRUCTURAL_VALUE => decode_move_structural_value(reader)?,
        STRUCTURAL_CASE_LEAF_COPY => decode_structural_case_leaf_copy(reader)?,
        WRITE_ONLY_PRIMITIVE_STORE => decode_write_only_primitive_store(reader)?,
        BYTE_SEQUENCE_STORE => OperationKind::ByteSequenceStore {
            destination: reader.id("PlaceId")?,
            source: reader.id("PlaceId")?,
            length: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE => {
            decode_structural_byte_sequence_field_store(reader)?
        }
        STRUCTURAL_BYTE_VIEW_FIELD_STORE => decode_structural_byte_view_field_store(reader, false)?,
        PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE => {
            decode_structural_byte_view_field_store(reader, true)?
        }
        STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH => {
            decode_structural_byte_sequence_field_length(reader)?
        }
        STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE => {
            decode_structural_byte_sequence_field_byte_store(reader)?
        }
        STRUCTURAL_BYTE_SEQUENCE_FIELD_READ => decode_structural_byte_sequence_field_read(reader)?,
        STRUCTURAL_SCALAR_FIELD_STORE => decode_structural_scalar_field_store(reader)?,
        RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE => {
            decode_range_checked_structural_scalar_field_store(reader)?
        }
        MOVE_STRUCTURAL_FIELD => decode_move_structural_field(reader)?,
        STORE_STRUCTURAL_FIELD => {
            decode_store_structural_field(reader, terminal_psi::StructuralStoreDisposition::Vacant)?
        }
        REPLACE_DISCARDABLE_STRUCTURAL_FIELD => decode_store_structural_field(
            reader,
            terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
        )?,
        ESTABLISH_ACTIVATION_CLAIM => OperationKind::EstablishActivationClaim {
            bound: reader.u64()?,
            committed: reader.id("ValueId")?,
            backing_type: reader.id("StructuralTypeId")?,
            established_case: reader.id("StructuralCaseId")?,
            rejected_case: reader.id("StructuralCaseId")?,
        },
        RELEASE_ACTIVATION_CLAIM => OperationKind::ReleaseActivationClaim {
            source: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
            claim: reader.id("ClaimId")?,
        },
        SPLIT_ACTIVATION_CLAIM => OperationKind::SplitActivationClaim {
            source: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
            claim: reader.id("ClaimId")?,
            bound_at: reader.u64()?,
            committed_at: reader.id("ValueId")?,
            covered_case: reader.id("StructuralCaseId")?,
            rejected_case: reader.id("StructuralCaseId")?,
        },
        MERGE_ACTIVATION_CLAIM => OperationKind::MergeActivationClaim {
            lower: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
            upper: terminal_psi::StructuralArgument {
                place: reader.id("PlaceId")?,
                access: super::structural_signature_wire::decode_structural_access(reader)?,
                path: decode_structural_path(reader)?,
            },
            lower_claim: reader.id("ClaimId")?,
            upper_claim: reader.id("ClaimId")?,
            covered_case: reader.id("StructuralCaseId")?,
            rejected_case: reader.id("StructuralCaseId")?,
        },
        ESTABLISH_SCALAR_ARRAY => decode_establish_scalar_array(reader)?,
        ESTABLISH_SCALAR_CASE => decode_establish_scalar_case(reader)?,
        ESTABLISH_BYTE_SEQUENCE_CONCATENATION => {
            decode_establish_byte_sequence_concatenation(reader)?
        }
        BYTE_SEQUENCE_PREDICATE => decode_byte_sequence_predicate(reader)?,
        BYTE_SEQUENCE_EQUAL => decode_byte_sequence_equal(reader)?,
        ESTABLISH_BYTE_SEQUENCE_LITERAL => decode_establish_byte_sequence_literal(reader)?,
        INTEGER_CONSTANT => decode_integer_constant(reader)?,
        BOOLEAN_CONSTANT => decode_boolean_constant(reader)?,
        IEEE_FLOAT_CONSTANT => decode_ieee_float_constant(reader)?,
        IEEE_FLOAT_COMPARE => decode_ieee_float_compare(reader)?,
        SCALAR_REPRESENTATION_VIEW => decode_scalar_representation_view(reader)?,
        LANDED_SCALAR_REPRESENTATION_VIEW => decode_landed_scalar_representation_view(reader)?,
        NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD => {
            decode_nearest_ieee_float_fused_multiply_add(reader)?
        }
        BOOLEAN_STRUCTURAL_FIELD => decode_boolean_structural_field(reader)?,
        SCALAR_STRUCTURAL_FIELD => decode_scalar_structural_field(reader)?,
        WRAPPING_INTEGER_ADD => decode_wrapping_integer_add(reader)?,
        SATURATING_INTEGER_ADD => decode_saturating_integer_add(reader)?,
        WRAPPING_INTEGER_SUBTRACT => decode_wrapping_integer_subtract(reader)?,
        SATURATING_INTEGER_SUBTRACT => decode_saturating_integer_subtract(reader)?,
        WRAPPING_INTEGER_MULTIPLY => decode_wrapping_integer_multiply(reader)?,
        SATURATING_INTEGER_MULTIPLY => decode_saturating_integer_multiply(reader)?,
        BOOLEAN_NOT => decode_boolean_not(reader)?,
        BOOLEAN_EQUAL => decode_boolean_equal(reader)?,
        INTEGER_EQUAL => decode_integer_equal(reader)?,
        INTEGER_LESS_THAN => decode_integer_less_than(reader)?,
        INTEGER_LESS_OR_EQUAL => decode_integer_less_or_equal(reader)?,
        INTEGER_BITWISE_AND => decode_integer_bitwise_and(reader)?,
        INTEGER_BITWISE_OR => decode_integer_bitwise_or(reader)?,
        INTEGER_BITWISE_XOR => decode_integer_bitwise_xor(reader)?,
        WRAPPING_INTEGER_SHIFT_LEFT => decode_wrapping_integer_shift_left(reader)?,
        WRAPPING_INTEGER_SHIFT_RIGHT => decode_wrapping_integer_shift_right(reader)?,
        INTEGER_BITWISE_NOT => decode_integer_bitwise_not(reader)?,
        INTEGER_WIDEN => decode_integer_widen(reader)?,
        INTEGER_EXACT_CAST => decode_integer_exact_cast(reader)?,
        EXACT_INTEGER_SHIFT_RIGHT => decode_exact_integer_shift_right(reader)?,
        EXACT_INTEGER_SHIFT_LEFT => decode_exact_integer_shift_left(reader)?,
        EXACT_INTEGER_ADD => decode_exact_integer_add(reader)?,
        EXACT_INTEGER_SUBTRACT => decode_exact_integer_subtract(reader)?,
        EXACT_INTEGER_MULTIPLY => decode_exact_integer_multiply(reader)?,
        EXACT_INTEGER_DIVIDE => decode_exact_integer_divide(reader)?,
        EXACT_INTEGER_REMAINDER => decode_exact_integer_remainder(reader)?,
        WRAPPING_INTEGER_DIVIDE => decode_wrapping_integer_divide(reader)?,
        WRAPPING_INTEGER_REMAINDER => decode_wrapping_integer_remainder(reader)?,
        SATURATING_INTEGER_DIVIDE => decode_saturating_integer_divide(reader)?,
        SATURATING_INTEGER_REMAINDER => decode_saturating_integer_remainder(reader)?,
        CALL => call_operations::decode_call(reader)?,
        CALL_UNIT => call_operations::decode_call_unit(reader)?,
        SELECT_IMPLEMENTATION => call_operations::decode_select_implementation(reader)?,
        CALL_DYNAMIC => call_operations::decode_call_dynamic(reader)?,
        BOUNDARY_CALL => call_operations::decode_boundary_call(reader)?,
        PORT_WRITE => call_operations::decode_port_write(reader)?,
        REGISTER_ACCESS => call_operations::decode_register_access(reader, true)?,
        REGISTER_OBSERVATION => call_operations::decode_register_access(reader, false)?,
        ESTABLISH_ZEROED_STRUCTURAL => decode_establish_zeroed_structural(reader)?,
        ESTABLISH_STRUCTURAL_LOCAL => decode_establish_structural_local(reader)?,
        ESTABLISH_TRIVIAL_AFFINE_LOCAL => decode_establish_trivial_affine_local(reader)?,
        ESTABLISH_STRUCTURAL_ARRAY => decode_establish_structural_array(reader)?,
        ESTABLISH_RECORD => decode_establish_record(reader)?,
        ESTABLISH_STRUCTURAL_CASE => decode_establish_structural_case(reader)?,
        CALL_STRUCTURAL_SCALAR => call_operations::decode_call_structural_scalar(reader)?,
        CALL_STRUCTURAL => call_operations::decode_call_structural(reader)?,
        CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS => {
            call_operations::decode_call_structural_with_scalar_arguments(reader)?
        }
        TRAPPING_INTEGER => decode_trapping_integer(reader)?,
        FLOAT_RESULT_POLICY => decode_float_result_policy(reader)?,
        ATOMIC_FENCE => decode_atomic_fence(reader)?,
        ATOMIC_ACCESS => decode_atomic_access(reader, false)?,
        RESTRICTED_ATOMIC_ACCESS => decode_atomic_access(reader, true)?,
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
