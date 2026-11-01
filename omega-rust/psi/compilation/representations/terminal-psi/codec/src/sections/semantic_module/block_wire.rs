//! Canonical terminal block, operation, and terminator wire format.
//!
//! This module owns operation-result and operation-kind rows plus the exact
//! terminal control-flow envelope. Shared structural paths, call arguments,
//! contracts, and declaration primitives remain sibling- or parent-owned.
//!
//! `encode_block` and `decode_block` are the entries. An operation kind is one
//! row of `operation_wire!` below when its tag is fixed; kinds whose tag or
//! layout depends on their contents keep explicit arms that call their family
//! (`storage_operations`, `call_operations`, `scalar_operations`,
//! `atomic_operations`). The terminator envelope is `terminator_wire`.

mod atomic_operations;
mod call_operations;
mod operation_tags;
mod scalar_operations;
mod storage_operations;
mod terminator_wire;

#[cfg(test)]
mod owned_value_tests;

use terminal_psi::{Block, Operation, OperationKind, OperationResult};

use super::CodecError;
use super::machine_wire::{
    decode_declaration, decode_declarations, encode_declaration, encode_declarations,
};
use super::proof_term_wire::{decode_proof_formals, encode_proof_formals};
use super::structural_result_wire::{decode_operation_result, encode_operation_result};
use super::structural_signature_wire::{
    decode_structural_parameters, encode_structural_parameters,
};
use super::wire::{Reader, Writer};
use super::wire_fields;
use crate::sections::semantic_module::wire::decode_counted;

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
/// kind as its `operation_wire!` row or explicit arm spells it.
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
    encode_operation_kind(writer, operation.kind.clone())?;
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
    let kind = decode_operation_kind(reader)?;
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

/// One row per fixed-tag operation: its tag, then every recorded field in wire
/// order with its spelling. Encoding and decoding both expand from the row, so
/// neither side can carry a field the other drops; a row names every field of
/// its variant, so a new field fails to compile until its spelling is chosen.
/// A paired row selects its second tag exactly when the trailing optional
/// obligation is present. Operations whose tag depends on other contents keep
/// explicit arms, inside the same exhaustive match.
macro_rules! operation_wire {
    (
        rows { $($tag:ident => $variant:ident { $($field:ident: $kind:ident),* $(,)? }),* $(,)? }
        paired {
            $($absent:ident | $present:ident => $paired:ident {
                $($paired_field:ident: $paired_kind:ident),* ; $optional:ident
            }),* $(,)?
        }
        encode($writer:ident) { $($encode_pattern:pat => $encode_body:expr),* $(,)? }
        decode($reader:ident) { $($decode_tag:pat => $decode_body:expr),* $(,)? }
    ) => {
        fn encode_operation_kind(
            $writer: &mut Writer,
            kind: OperationKind,
        ) -> Result<(), CodecError> {
            match kind {
                $(OperationKind::$variant { $($field),* } => {
                    $writer.u8(operation_tags::$tag);
                    $(<wire_fields::$kind as wire_fields::Field<_>>::write($writer, &$field)?;)*
                })*
                $(OperationKind::$paired { $($paired_field,)* $optional } => {
                    $writer.u8(if $optional.is_some() {
                        operation_tags::$present
                    } else {
                        operation_tags::$absent
                    });
                    $(<wire_fields::$paired_kind as wire_fields::Field<_>>::write(
                        $writer,
                        &$paired_field,
                    )?;)*
                    if let Some(obligation) = $optional {
                        $writer.id(obligation);
                    }
                })*
                $($encode_pattern => {
                    $encode_body;
                })*
            }
            Ok(())
        }

        fn decode_operation_kind($reader: &mut Reader<'_>) -> Result<OperationKind, CodecError> {
            Ok(match $reader.u8()? {
                $(operation_tags::$tag => OperationKind::$variant {
                    $($field: <wire_fields::$kind as wire_fields::Field<_>>::read($reader)?),*
                },)*
                $(operation_tags::$absent => OperationKind::$paired {
                    $($paired_field: <wire_fields::$paired_kind as wire_fields::Field<_>>::read($reader)?,)*
                    $optional: None,
                },
                operation_tags::$present => OperationKind::$paired {
                    $($paired_field: <wire_fields::$paired_kind as wire_fields::Field<_>>::read($reader)?,)*
                    $optional: Some($reader.id("ObligationId")?),
                },)*
                $($decode_tag => $decode_body,)*
                tag => return Err(CodecError::InvalidTag("OperationKind", tag)),
            })
        }
    };
}

operation_wire! {
    rows {
        READ_SCALAR_APPLICATION => ReadScalarApplication { application: Id },
        ESTABLISH_NATIVE_CODE_LAYOUT => EstablishNativeCodeLayout { machine: Id },
        STORAGE_ADDRESS => StorageAddress { source: Argument },
        ESTABLISH_ACCESS_REFERENCE => EstablishAccessReference {
            source: Argument,
            resident: Resident,
        },
        ESTABLISH_PRIMITIVE_LOCAL => EstablishPrimitiveLocal {
            value: Id,
            range_obligation: OptionalId,
        },
        STRUCTURAL_CASE_MEMBERSHIP => StructuralCaseMembership { source: Id, path: Path, case: Id },
        STRUCTURAL_LEAF_COPY => StructuralLeafCopy { source: Id, path: Path },
        MOVE_STRUCTURAL_VALUE => MoveStructuralValue { source: Argument },
        BYTE_SEQUENCE_SUBSLICE => ByteSequenceSubslice {
            source: Argument,
            start: Id,
            end: Id,
            length: Id,
            obligation: Id,
        },
        BYTE_SEQUENCE_WRITE => ByteSequenceWrite {
            destination: Id,
            index: Id,
            value: Id,
            length: Id,
            obligation: Id,
        },
        ELEMENT_VIEW_WRITE => ElementViewWrite {
            destination: Argument,
            path: Path,
            index: Id,
            value: Id,
            length: Id,
            obligation: Id,
        },
        BYTE_SEQUENCE_READ => ByteSequenceRead {
            source: Id,
            index: Id,
            length: Id,
            obligation: Id,
        },
        BYTE_SEQUENCE_LENGTH => ByteSequenceLength { source: Argument },
        BYTE_SEQUENCE_STORE => ByteSequenceStore {
            destination: Id,
            source: Id,
            length: Id,
            obligation: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_STORE => StructuralByteSequenceFieldStore {
            destination: Id,
            path: Path,
            field: Id,
            source: Id,
            length: Id,
            obligation: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_LENGTH => StructuralByteSequenceFieldLength {
            source: Id,
            path: Path,
            field: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_BYTE_STORE => StructuralByteSequenceFieldByteStore {
            destination: Id,
            path: Path,
            field: Id,
            index: Id,
            value: Id,
            length: Id,
            obligation: Id,
        },
        STRUCTURAL_BYTE_SEQUENCE_FIELD_READ => StructuralByteSequenceFieldRead {
            source: Id,
            path: Path,
            field: Id,
            index: Id,
            length: Id,
            obligation: Id,
        },
        MOVE_STRUCTURAL_FIELD => MoveStructuralField { source: Id, path: Path, field: OptionalField },
        ESTABLISH_STRUCTURAL_ARRAY => EstablishStructuralArray { elements: Arguments },
        ESTABLISH_ACTIVATION_CLAIM => EstablishActivationClaim {
            bound: U64,
            committed: Id,
            backing_type: Id,
            established_case: Id,
            rejected_case: Id,
        },
        SPLIT_ACTIVATION_CLAIM => SplitActivationClaim {
            source: Argument,
            claim: Id,
            bound_at: U64,
            committed_at: Id,
            covered_case: Id,
            rejected_case: Id,
        },
        MERGE_ACTIVATION_CLAIM => MergeActivationClaim {
            lower: Argument,
            upper: Argument,
            lower_claim: Id,
            upper_claim: Id,
            covered_case: Id,
            rejected_case: Id,
        },
        RELEASE_ACTIVATION_CLAIM => ReleaseActivationClaim { source: Argument, claim: Id },
        ESTABLISH_SCALAR_ARRAY => EstablishScalarArray { elements: Ids },
        ESTABLISH_SCALAR_CASE => EstablishScalarCase { result_case: Id, fields: ScalarCaseFields },
        ESTABLISH_BYTE_SEQUENCE_CONCATENATION => EstablishByteSequenceConcatenation {
            left: ByteOperand,
            right: ByteOperand,
            capacity_obligation: Id,
        },
        BYTE_SEQUENCE_PREDICATE => ByteSequencePredicate { operand: ByteOperand, predicate: Predicate },
        BYTE_SEQUENCE_EQUAL => ByteSequenceEqual { left: ByteOperand, right: ByteOperand },
        ESTABLISH_BYTE_SEQUENCE_LITERAL => EstablishByteSequenceLiteral {
            destination: Id,
            bytes: Bytes,
            qualifications: Ids,
        },
        ESTABLISH_ELEMENT_VIEW => EstablishElementView {
            destination: Id,
            source: Argument,
            element: Id,
            region: Region,
        },
        ESTABLISH_MUTABLE_ELEMENT_VIEW => EstablishMutableElementView {
            destination: Id,
            source: Argument,
            element: Id,
            region: Region,
        },
        ELEMENT_VIEW_LENGTH => ElementViewLength { source: Argument },
        ELEMENT_VIEW_SUBSLICE => ElementViewSubslice {
            source: Argument,
            start: Id,
            end: Id,
            length: Id,
            obligation: Id,
        },
        ESTABLISH_ZEROED_STRUCTURAL => EstablishZeroedStructural {},
        ESTABLISH_STRUCTURAL_LOCAL => EstablishStructuralLocal {},
        ESTABLISH_TRIVIAL_AFFINE_LOCAL => EstablishTrivialAffineLocal { destination: Id },
        ESTABLISH_RECORD => EstablishRecord { fields: RecordFields },
        ESTABLISH_STRUCTURAL_CASE => EstablishStructuralCase {
            result_case: Id,
            fields: RecordFields,
        },
        CALL => Call {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        CALL_UNIT => CallUnit {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        CALL_STRUCTURAL_SCALAR => CallStructuralScalar {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        CALL_STRUCTURAL_WITH_SCALAR_ARGUMENTS => CallStructuralWithScalarArguments {
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
            selected_evidence: SelectedEvidence,
        },
        CALL_STRUCTURAL => CallStructural {
            callee: Id,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
            selected_evidence: SelectedEvidence,
        },
        CALL_DYNAMIC => CallDynamic {
            selector: Id,
            interface: U32,
            slot: U32,
            arguments: Ids,
            family_actuals: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        BOUNDARY_CALL => BoundaryCall {
            boundary: Id,
            arguments: Ids,
            structural_arguments: Arguments,
            completion_receipts: CompletionReceipts,
            requirement_obligations: Ids,
        },
        SELECT_IMPLEMENTATION => SelectImplementation { table: U32 },
        PORT_WRITE => PortWrite { service: Id, port: U16, value: U8 },
        INTEGER_CONSTANT => IntegerConstant { value: Integer },
        BOOLEAN_CONSTANT => BooleanConstant { value: Flag },
        IEEE_FLOAT_CONSTANT => IeeeFloatConstant { value: Float },
        IEEE_FLOAT_COMPARE => IeeeFloatCompare {
            comparison: FloatComparison,
            left: Id,
            right: Id,
        },
        NEAREST_IEEE_FLOAT_FUSED_MULTIPLY_ADD => NearestIeeeFloatFusedMultiplyAdd {
            left: Id,
            right: Id,
            addend: Id,
        },
        BOOLEAN_STRUCTURAL_FIELD => BooleanStructuralField { source: Id, path: FieldPath, field: Id },
        SCALAR_STRUCTURAL_FIELD => ScalarStructuralField { source: Id, path: FieldPath, field: Id },
        BOOLEAN_NOT => BooleanNot { operand: Id },
        BOOLEAN_EQUAL => BooleanEqual { left: Id, right: Id },
        INTEGER_EQUAL => IntegerEqual { left: Id, right: Id },
        INTEGER_LESS_THAN => IntegerLessThan { left: Id, right: Id },
        INTEGER_LESS_OR_EQUAL => IntegerLessOrEqual { left: Id, right: Id },
        INTEGER_BITWISE_NOT => IntegerBitwiseNot { operand: Id },
        INTEGER_WIDEN => IntegerWiden { operand: Id },
        INTEGER_EXACT_CAST => IntegerExactCast { operand: Id, obligation: Id },
        INTEGER_BITWISE_AND => IntegerBitwiseAnd { left: Id, right: Id },
        INTEGER_BITWISE_OR => IntegerBitwiseOr { left: Id, right: Id },
        INTEGER_BITWISE_XOR => IntegerBitwiseXor { left: Id, right: Id },
        WRAPPING_INTEGER_SHIFT_LEFT => WrappingIntegerShiftLeft { value: Id, count: Id },
        WRAPPING_INTEGER_SHIFT_RIGHT => WrappingIntegerShiftRight { value: Id, count: Id },
        EXACT_INTEGER_SHIFT_LEFT => ExactIntegerShiftLeft { value: Id, count: Id, obligation: Id },
        EXACT_INTEGER_SHIFT_RIGHT => ExactIntegerShiftRight { value: Id, count: Id, obligation: Id },
        EXACT_INTEGER_ADD => ExactIntegerAdd { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_SUBTRACT => ExactIntegerSubtract { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_MULTIPLY => ExactIntegerMultiply { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_DIVIDE => ExactIntegerDivide { left: Id, right: Id, obligation: Id },
        EXACT_INTEGER_REMAINDER => ExactIntegerRemainder { left: Id, right: Id, obligation: Id },
        WRAPPING_INTEGER_DIVIDE => WrappingIntegerDivide { left: Id, right: Id, obligation: Id },
        WRAPPING_INTEGER_REMAINDER => WrappingIntegerRemainder { left: Id, right: Id, obligation: Id },
        SATURATING_INTEGER_DIVIDE => SaturatingIntegerDivide { left: Id, right: Id, obligation: Id },
        SATURATING_INTEGER_REMAINDER => SaturatingIntegerRemainder {
            left: Id,
            right: Id,
            obligation: Id,
        },
        WRAPPING_INTEGER_ADD => WrappingIntegerAdd { left: Id, right: Id },
        SATURATING_INTEGER_ADD => SaturatingIntegerAdd { left: Id, right: Id },
        WRAPPING_INTEGER_SUBTRACT => WrappingIntegerSubtract { left: Id, right: Id },
        SATURATING_INTEGER_SUBTRACT => SaturatingIntegerSubtract { left: Id, right: Id },
        WRAPPING_INTEGER_MULTIPLY => WrappingIntegerMultiply { left: Id, right: Id },
        SATURATING_INTEGER_MULTIPLY => SaturatingIntegerMultiply { left: Id, right: Id },
    }
    paired {
        STRUCTURAL_BYTE_VIEW_FIELD_STORE | PREDICATE_CHECKED_STRUCTURAL_BYTE_VIEW_FIELD_STORE
            => StructuralByteViewFieldStore {
                destination: Id, path: Path, field: OptionalField, source: Id; predicate_obligation
            },
        STRUCTURAL_SCALAR_FIELD_STORE | RANGE_CHECKED_STRUCTURAL_SCALAR_FIELD_STORE
            => StructuralScalarFieldStore {
                destination: Id, path: Path, field: Id, value: Id; range_obligation
            },
        SCALAR_REPRESENTATION_VIEW | LANDED_SCALAR_REPRESENTATION_VIEW
            => ScalarRepresentationView { operand: Id; landing_obligation },
    }
    encode(writer) {
        OperationKind::EstablishReference { source, storage } => {
            storage_operations::encode_establish_reference(writer, source, storage)?
        },
        OperationKind::ReleaseReference { source, path } => {
            storage_operations::encode_release_reference(writer, source, path)?
        },
        OperationKind::PrimitiveScalarRead { source, path } => {
            storage_operations::encode_primitive_scalar_read(writer, source, path)?
        },
        OperationKind::TransformContent {
            source,
            parents,
            derivations,
        } => {
            writer.u8(operation_tags::TRANSFORM_CONTENT);
            super::structural_place_wire::encode_structural_arguments(writer, &[source])?;
            super::structural_place_wire::encode_structural_arguments(writer, &parents)?;
            super::content_wire::encode_content_transformations(writer, &derivations)?;
        },
        OperationKind::StructuralCaseLeafCopy { source, path } => {
            storage_operations::encode_structural_case_leaf_copy(writer, source, &path)?
        },
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
        OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        } => storage_operations::encode_element_view_read(
            writer, source, index, length, obligation, path,
        )?,
        OperationKind::RegisterAccess { service, event } => {
            call_operations::encode_register_access(writer, service, event)?
        },
        OperationKind::TrappingInteger { operation } => {
            scalar_operations::encode_trapping_integer(writer, operation)?
        },
        OperationKind::FloatResultPolicy { policy } => {
            scalar_operations::encode_float_result_policy(writer, &policy)?
        },
        OperationKind::AtomicFence { ordering } => {
            atomic_operations::encode_atomic_fence(writer, ordering)
        },
        OperationKind::AtomicAccess {
            place,
            path,
            field,
            event,
            resident_obligation,
        } => atomic_operations::encode_atomic_access(
            writer,
            place,
            &path,
            field,
            event,
            resident_obligation,
        )?,
    }
    decode(reader) {
        operation_tags::ESTABLISH_REFERENCE => {
            storage_operations::decode_establish_reference(reader, false)?
        },
        operation_tags::ESTABLISH_STORAGE_VIEW_REFERENCE => {
            storage_operations::decode_establish_reference(reader, true)?
        },
        operation_tags::RELEASE_REFERENCE => {
            storage_operations::decode_release_reference(reader, false)?
        },
        operation_tags::PROJECTED_RELEASE_REFERENCE => {
            storage_operations::decode_release_reference(reader, true)?
        },
        operation_tags::PRIMITIVE_SCALAR_READ => OperationKind::PrimitiveScalarRead {
            path: Vec::new(),
            source: reader.id("PlaceId")?,
        },
        operation_tags::PROJECTED_PRIMITIVE_SCALAR_READ => {
            storage_operations::decode_projected_primitive_scalar_read(reader)?
        },
        operation_tags::TRANSFORM_CONTENT => {
            let mut sources = super::structural_place_wire::decode_structural_arguments(reader)?;
            if sources.len() != 1 {
                return Err(CodecError::MalformedStructuralFoundation(
                    "content transformation requires one payload",
                ));
            }
            let source = sources.remove(0);
            OperationKind::TransformContent {
                source,
                parents: super::structural_place_wire::decode_structural_arguments(reader)?,
                derivations: super::content_wire::decode_content_transformations(reader)?,
            }
        },
        operation_tags::STRUCTURAL_CASE_LEAF_COPY => {
            let (source, path) =
                super::structural_field_wire::decode_canonical_structural_field(reader)?;
            OperationKind::StructuralCaseLeafCopy { source, path }
        },
        operation_tags::WRITE_ONLY_PRIMITIVE_STORE => OperationKind::WriteOnlyPrimitiveStore {
            path: Vec::new(),
            destination: reader.id("PlaceId")?,
            value: reader.id("ValueId")?,
            range_obligation: None,
        },
        operation_tags::PROJECTED_WRITE_ONLY_PRIMITIVE_STORE => {
            storage_operations::decode_projected_write_only_primitive_store(reader)?
        },
        operation_tags::RESTRICTED_WRITE_ONLY_PRIMITIVE_STORE => {
            OperationKind::WriteOnlyPrimitiveStore {
                destination: reader.id("PlaceId")?,
                path: super::structural_place_wire::decode_structural_path(reader)?,
                value: reader.id("ValueId")?,
                range_obligation: Some(reader.id("ObligationId")?),
            }
        },
        operation_tags::STORE_STRUCTURAL_FIELD => storage_operations::decode_store_structural_field(
            reader,
            terminal_psi::StructuralStoreDisposition::Vacant,
        )?,
        operation_tags::REPLACE_DISCARDABLE_STRUCTURAL_FIELD => {
            storage_operations::decode_store_structural_field(
                reader,
                terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
            )?
        },
        operation_tags::ELEMENT_VIEW_READ => OperationKind::ElementViewRead {
            source: reader.id("PlaceId")?,
            index: reader.id("ValueId")?,
            length: reader.id("ValueId")?,
            obligation: reader.id("ObligationId")?,
            path: Vec::new(),
        },
        operation_tags::PROJECTED_ELEMENT_VIEW_READ => {
            storage_operations::decode_projected_element_view_read(reader)?
        },
        operation_tags::REGISTER_ACCESS => call_operations::decode_register_access(reader, true)?,
        operation_tags::REGISTER_OBSERVATION => {
            call_operations::decode_register_access(reader, false)?
        },
        operation_tags::TRAPPING_INTEGER => scalar_operations::decode_trapping_integer(reader)?,
        operation_tags::FLOAT_RESULT_POLICY => {
            scalar_operations::decode_float_result_policy(reader)?
        },
        operation_tags::ATOMIC_FENCE => atomic_operations::decode_atomic_fence(reader)?,
        operation_tags::ATOMIC_ACCESS => atomic_operations::decode_atomic_access(reader, false)?,
        operation_tags::RESTRICTED_ATOMIC_ACCESS => {
            atomic_operations::decode_atomic_access(reader, true)?
        },
    }
}
