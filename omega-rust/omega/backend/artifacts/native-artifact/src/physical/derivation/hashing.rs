//! The SHA-256 encodings every identity in the derivation is built from.

pub(crate) use crate::NormalizedForeignCallbackRelocation;
use machine_code::{BoundaryExecutionRecord, PortEffectRecord};
use object_file::{RelocationKind, RelocationOrigin};
use semantic_vocabulary::{IntegerSign, ScalarType};
use sha2::Digest;
use sha2::Sha256;
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::{
    BoundaryRealization, CallSiteOwner, CompilerBuiltinExecution, CompletionClaimSource,
};

use crate::physical::model::NativePhysicalEvidenceGapSubject;

pub(crate) fn hash_structural_path(
    digest: &mut Sha256,
    path: &[terminal_psi::StructuralPathSegment],
) {
    digest.update((path.len() as u64).to_le_bytes());
    for segment in path {
        match segment {
            terminal_psi::StructuralPathSegment::Referent => digest.update([3]),
            terminal_psi::StructuralPathSegment::Field(identity) => {
                digest.update([1]);
                hash_bytes(digest, identity.as_bytes());
            }
            terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                digest.update([2]);
                digest.update(index.to_le_bytes());
            }
        }
    }
}

pub(crate) fn hash_sum_layout(
    digest: &mut Sha256,
    layout: &calling_conventions::ConventionalSumLayout,
) {
    hash_integer_shape(digest, layout.shape);
    digest.update(layout.tag_byte_offset.to_le_bytes());
    hash_integer_shape(digest, layout.tag_shape);
    hash_packed_fields(digest, &layout.common_fields);
    digest.update(layout.payload_byte_offset.to_le_bytes());
    digest.update((layout.cases.len() as u64).to_le_bytes());
    for case in &layout.cases {
        hash_packed_fields(digest, &case.fields);
    }
}

fn hash_packed_fields(digest: &mut Sha256, fields: &[calling_conventions::PackedFieldLayout]) {
    digest.update((fields.len() as u64).to_le_bytes());
    for field in fields {
        digest.update(field.byte_offset.to_le_bytes());
        hash_integer_shape(digest, field.shape);
    }
}

fn hash_integer_shape(digest: &mut Sha256, shape: calling_conventions::ValueShape) {
    debug_assert_eq!(shape.class, calling_conventions::ValueClass::Integer);
    digest.update([1]);
    digest.update(shape.byte_size.to_le_bytes());
    digest.update(shape.alignment.to_le_bytes());
}

pub(crate) fn hash_port_effect_record(digest: &mut Sha256, effect: &PortEffectRecord) {
    digest.update(effect.psi_operation.get().to_le_bytes());
    digest.update(effect.service.get().to_le_bytes());
    digest.update(effect.port.to_le_bytes());
    digest.update([effect.value]);
    digest.update(canonical_usize(effect.operation_ordinal));
    digest.update(canonical_usize(effect.code_offset));
    digest.update(canonical_usize(effect.byte_count));
}

/// Hash the complete retained settlement row so no realization, argument,
/// completion, result, or coordinate field can be substituted underneath a
/// parent identity.
pub(crate) fn hash_boundary_settlement_record(
    digest: &mut Sha256,
    settlement: &machine_code::BoundarySettlementRecord,
) -> Result<(), &'static str> {
    digest.update(settlement.psi_operation.get().to_le_bytes());
    digest.update(settlement.boundary.get().to_le_bytes());
    match settlement.execution {
        BoundaryExecutionRecord::AdmittedProvider(record) => {
            digest.update([1]);
            digest.update(record.provider_plan_report_identity.to_le_bytes());
            digest.update(record.provider_execution_report_identity.to_le_bytes());
            digest.update(record.provider_execution_report_fingerprint.to_le_bytes());
            digest.update(record.normalized_root_report_identity.to_le_bytes());
            digest.update(record.boundary_contract_report_fingerprint.to_le_bytes());
        }
        BoundaryExecutionRecord::CompilerBuiltin(execution) => {
            digest.update([2]);
            digest.update([compiler_builtin_execution_tag(execution)]);
        }
    }
    hash_boundary_realization(digest, &settlement.realization);
    digest.update(canonical_usize(settlement.scalar_arguments.len()));
    for argument in &settlement.scalar_arguments {
        digest.update(argument.source_value.get().to_le_bytes());
        hash_scalar_type(digest, argument.scalar_type);
        hash_integer_value(digest, argument.immediate);
        hash_machine_register(digest, argument.destination);
    }
    digest.update(canonical_usize(settlement.runtime_scalar_arguments.len()));
    for argument in &settlement.runtime_scalar_arguments {
        digest.update(argument.parameter_index.to_le_bytes());
        hash_internal_scalar_argument_source(digest, &argument.source);
        hash_value_placement(digest, &argument.placement);
        digest.update(canonical_usize(argument.code_offset));
        digest.update(canonical_usize(argument.byte_count));
    }
    digest.update(canonical_usize(settlement.arguments.len()));
    for argument in &settlement.arguments {
        hash_structural_argument(digest, argument);
    }
    digest.update(canonical_usize(settlement.byte_sequence_arguments.len()));
    for custody in &settlement.byte_sequence_arguments {
        hash_structural_argument(digest, &custody.argument);
        digest.update(custody.literal_operation.get().to_le_bytes());
        let declaration = terminal_codec::encode_structural_type_declaration(
            &custody.structural_type,
        )
        .map_err(|_| "installed D41 byte-sequence declaration cannot be encoded canonically")?;
        hash_bytes(digest, &declaration);
        hash_bytes(digest, &custody.bytes);
        digest.update(canonical_usize(custody.code_offset));
        digest.update(canonical_usize(custody.code_byte_count));
        digest.update(canonical_usize(custody.data_offset));
        digest.update(canonical_usize(custody.data_byte_count));
    }
    digest.update(canonical_usize(settlement.completion_claim_sources.len()));
    for source in &settlement.completion_claim_sources {
        hash_claim_source(digest, source);
    }
    digest.update(canonical_usize(settlement.completion_receipts.len()));
    for receipt in &settlement.completion_receipts {
        digest.update(receipt.claim.get().to_le_bytes());
        digest.update(receipt.argument_index.to_le_bytes());
    }
    digest.update(canonical_usize(
        settlement.completion_provider_custody.len(),
    ));
    for binding in &settlement.completion_provider_custody {
        hash_claim_source(digest, &binding.source);
        digest.update(binding.receipt.claim.get().to_le_bytes());
        digest.update(binding.receipt.argument_index.to_le_bytes());
        let execution = binding.provider_execution;
        digest.update(execution.provider_plan_report_identity.to_le_bytes());
        digest.update(execution.provider_execution_report_identity.to_le_bytes());
        digest.update(
            execution
                .provider_execution_report_fingerprint
                .to_le_bytes(),
        );
        digest.update(execution.normalized_root_report_identity.to_le_bytes());
        digest.update(execution.boundary_contract_report_fingerprint.to_le_bytes());
    }
    hash_boundary_result_record(digest, &settlement.native_result)?;
    digest.update(canonical_usize(settlement.operation_ordinal));
    digest.update(canonical_usize(settlement.code_offset));
    digest.update(canonical_usize(settlement.byte_count));
    Ok(())
}

const fn compiler_builtin_execution_tag(execution: CompilerBuiltinExecution) -> u8 {
    match execution {
        CompilerBuiltinExecution::HostedExitProcessI32 => 1,
        CompilerBuiltinExecution::HostedReadByte => 2,
        CompilerBuiltinExecution::HostedWriteByteI32 => 3,
    }
}

fn hash_boundary_realization(digest: &mut Sha256, realization: &BoundaryRealization) {
    match realization {
        BoundaryRealization::MetadataOnlyPort(realization) => {
            digest.update([1]);
            digest.update(realization.effect_operation.get().to_le_bytes());
            digest.update(realization.service.get().to_le_bytes());
            digest.update(realization.port.to_le_bytes());
            digest.update([realization.value]);
        }
        BoundaryRealization::DirectPortReadU8(realization) => {
            digest.update([2]);
            digest.update(realization.service.get().to_le_bytes());
            digest.update(realization.port.to_le_bytes());
        }
        BoundaryRealization::LinuxWriteLine(_) => digest.update([3]),
        BoundaryRealization::ClaimCompletionOnly(_) => digest.update([4]),
        BoundaryRealization::HostedExitProcessI32(_) => digest.update([5]),
        BoundaryRealization::HostedReadByte(_) => digest.update([6]),
        BoundaryRealization::HostedWriteByteI32(_) => digest.update([7]),
    }
}

fn hash_boundary_result_record(
    digest: &mut Sha256,
    result: &machine_code::BoundaryResultRecord,
) -> Result<(), &'static str> {
    match result {
        machine_code::BoundaryResultRecord::Unit => digest.update([0]),
        machine_code::BoundaryResultRecord::Scalar(result) => {
            digest.update([1]);
            digest.update(result.value.get().to_le_bytes());
            hash_scalar_type(digest, result.scalar_type);
            hash_value_placement(digest, &result.placement);
            digest.update(result.return_edge.get().to_le_bytes());
        }
        machine_code::BoundaryResultRecord::Structural(result) => {
            digest.update([2]);
            hash_boundary_structural_result(digest, result)?;
        }
    }
    Ok(())
}

/// The complete structural-result record body shared by the hosted read-byte
/// parent identity and the installed-settlement record hash.
fn hash_boundary_structural_result(
    digest: &mut Sha256,
    result: &machine_code::BoundaryStructuralResultRecord,
) -> Result<(), &'static str> {
    digest.update(result.defining_operation.get().to_le_bytes());
    digest.update(result.result.place.get().to_le_bytes());
    digest.update(result.result.structural_type.get().to_le_bytes());
    digest.update([match result.result.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    }]);
    digest.update((result.result.qualifications.len() as u64).to_le_bytes());
    for domain in &result.result.qualifications {
        digest.update(domain.get().to_le_bytes());
    }
    digest.update((result.result.projected_qualifications.len() as u64).to_le_bytes());
    for qualification in &result.result.projected_qualifications {
        hash_structural_path(digest, &qualification.path);
        digest.update(qualification.domain.get().to_le_bytes());
    }
    digest.update((result.result.claims.len() as u64).to_le_bytes());
    for claim in &result.result.claims {
        digest.update(claim.claim.get().to_le_bytes());
        hash_structural_path(digest, &claim.path);
    }
    let declaration = terminal_codec::encode_structural_type_declaration(&result.declaration)
        .map_err(|_| "boundary structural result declaration cannot be encoded canonically")?;
    hash_bytes(digest, &declaration);
    hash_sum_layout(digest, &result.layout);
    digest.update(result.home_byte_offset.to_le_bytes());
    Ok(())
}

fn hash_scalar_type(digest: &mut Sha256, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => digest.update([1]),
        ScalarType::Integer(integer) => {
            digest.update([2]);
            digest.update([match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            }]);
            digest.update(integer.bits().to_le_bytes());
            digest.update([match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 1,
                semantic_vocabulary::IntegerCarrier::Address => 2,
            }]);
        }
        ScalarType::IeeeFloat(format) => {
            digest.update([3]);
            digest.update([match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 1,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 2,
            }]);
        }
    }
}

fn hash_integer_value(digest: &mut Sha256, value: semantic_vocabulary::IntegerValue) {
    match value {
        semantic_vocabulary::IntegerValue::Signed(value) => {
            digest.update([1]);
            digest.update(value.to_le_bytes());
        }
        semantic_vocabulary::IntegerValue::Unsigned(value) => {
            digest.update([2]);
            digest.update(value.to_le_bytes());
        }
    }
}

fn hash_machine_register(digest: &mut Sha256, register: calling_conventions::MachineRegister) {
    pub(crate) use calling_conventions::MachineRegister;
    match register {
        MachineRegister::X86Rax => digest.update([1]),
        MachineRegister::X86Rcx => digest.update([2]),
        MachineRegister::X86Rdx => digest.update([3]),
        MachineRegister::X86Rbx => digest.update([4]),
        MachineRegister::X86Rsp => digest.update([5]),
        MachineRegister::X86Rbp => digest.update([6]),
        MachineRegister::X86Rsi => digest.update([7]),
        MachineRegister::X86Rdi => digest.update([8]),
        MachineRegister::X86R8 => digest.update([9]),
        MachineRegister::X86R9 => digest.update([10]),
        MachineRegister::X86R10 => digest.update([11]),
        MachineRegister::X86R11 => digest.update([12]),
        MachineRegister::X86R12 => digest.update([13]),
        MachineRegister::X86R13 => digest.update([14]),
        MachineRegister::X86R14 => digest.update([15]),
        MachineRegister::X86R15 => digest.update([16]),
        MachineRegister::X86Xmm(index) => {
            digest.update([17]);
            digest.update([index]);
        }
        MachineRegister::Aarch64X(index) => {
            digest.update([18]);
            digest.update([index]);
        }
        MachineRegister::Aarch64V(index) => {
            digest.update([19]);
            digest.update([index]);
        }
    }
}

fn hash_value_placement(digest: &mut Sha256, placement: &calling_conventions::ValuePlacement) {
    hash_value_shape(digest, placement.shape);
    digest.update(canonical_usize(placement.locations.len()));
    for location in &placement.locations {
        match location {
            calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                digest.update([1]);
                hash_machine_register(digest, *register);
                digest.update(value_byte_offset.to_le_bytes());
                digest.update(byte_size.to_le_bytes());
            }
            calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                digest.update([2]);
                digest.update(stack_byte_offset.to_le_bytes());
                digest.update(value_byte_offset.to_le_bytes());
                digest.update(byte_size.to_le_bytes());
                digest.update(alignment.to_le_bytes());
            }
            calling_conventions::ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                digest.update([3]);
                match pointer {
                    calling_conventions::IndirectPointerLocation::Register(register) => {
                        digest.update([1]);
                        hash_machine_register(digest, *register);
                    }
                    calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        digest.update([2]);
                        digest.update(stack_byte_offset.to_le_bytes());
                        digest.update(alignment.to_le_bytes());
                    }
                }
                match copy_stack_byte_offset {
                    None => digest.update([0]),
                    Some(offset) => {
                        digest.update([1]);
                        digest.update(offset.to_le_bytes());
                    }
                }
                digest.update(byte_size.to_le_bytes());
                digest.update(alignment.to_le_bytes());
            }
        }
    }
}

fn hash_value_shape(digest: &mut Sha256, shape: calling_conventions::ValueShape) {
    match shape.class {
        calling_conventions::ValueClass::Integer => digest.update([1]),
        calling_conventions::ValueClass::Float => digest.update([2]),
        calling_conventions::ValueClass::BorrowedReference => digest.update([3]),
        calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
            digest.update([4]);
            digest.update([members]);
        }
        calling_conventions::ValueClass::SystemVAggregate { first, second } => {
            digest.update([5]);
            for class in [first, second] {
                digest.update([match class {
                    calling_conventions::SystemVEightbyteClass::Integer => 1,
                    calling_conventions::SystemVEightbyteClass::Sse => 2,
                }]);
            }
        }
    }
    digest.update(shape.byte_size.to_le_bytes());
    digest.update(shape.alignment.to_le_bytes());
}

pub(crate) fn hash_structural_argument(
    digest: &mut Sha256,
    argument: &terminal_psi::StructuralArgument,
) {
    digest.update(argument.place.get().to_le_bytes());
    digest.update([match argument.access {
        terminal_psi::StructuralAccess::Owned => 1,
        terminal_psi::StructuralAccess::SharedBorrow => 2,
        terminal_psi::StructuralAccess::MutableBorrow => 3,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => 4,
    }]);
    hash_structural_path(digest, &argument.path);
}

/// Hash one declared structural formal: its declared position, place,
/// self-ness, structural type, multiplicity, access, and exact qualification
/// rows. No declared field can drift underneath a retained argument binding.
pub(crate) fn hash_structural_parameter_declaration(
    digest: &mut Sha256,
    parameter: &terminal_psi::StructuralParameterDeclaration,
) {
    digest.update(parameter.place.get().to_le_bytes());
    digest.update(parameter.position.to_le_bytes());
    digest.update([u8::from(parameter.is_self)]);
    digest.update(parameter.structural_type.get().to_le_bytes());
    digest.update([match parameter.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    }]);
    digest.update([match parameter.access {
        terminal_psi::StructuralAccess::Owned => 1,
        terminal_psi::StructuralAccess::SharedBorrow => 2,
        terminal_psi::StructuralAccess::MutableBorrow => 3,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => 4,
    }]);
    digest.update(canonical_usize(parameter.qualifications.len()));
    for domain in &parameter.qualifications {
        digest.update(domain.get().to_le_bytes());
    }
    digest.update(canonical_usize(parameter.projected_qualifications.len()));
    for qualification in &parameter.projected_qualifications {
        hash_structural_path(digest, &qualification.path);
        digest.update(qualification.domain.get().to_le_bytes());
    }
}

fn hash_claim_source(digest: &mut Sha256, source: &CompletionClaimSource) {
    digest.update(source.claim.get().to_le_bytes());
    match &source.entry {
        None => digest.update([0]),
        Some(entry) => {
            digest.update([1]);
            digest.update(entry.claim.get().to_le_bytes());
            digest.update(entry.input.get().to_le_bytes());
            hash_structural_path(digest, &entry.path);
        }
    }
    match &source.content {
        None => digest.update([0]),
        Some(content) => {
            digest.update([1]);
            digest.update(content.claim.get().to_le_bytes());
            digest.update([match content.input.version {
                semantic_vocabulary::ContentPlaceVersion::Entry => 1,
                semantic_vocabulary::ContentPlaceVersion::Current => 2,
            }]);
            digest.update(content.input.root.get().to_le_bytes());
            digest.update(canonical_usize(content.input.segments.len()));
            for segment in &content.input.segments {
                match segment {
                    semantic_vocabulary::ContentPlaceSegment::Case(identity) => {
                        digest.update([1]);
                        hash_bytes(digest, identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::Field(identity) => {
                        digest.update([2]);
                        hash_bytes(digest, identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::FixedIndex(index) => {
                        digest.update([3]);
                        digest.update(index.to_le_bytes());
                    }
                }
            }
            digest.update(canonical_usize(content.projections.len()));
            for projection in &content.projections {
                digest.update(projection.projection.domain.get().to_le_bytes());
                digest.update(
                    projection
                        .projection
                        .projection_report_fingerprint
                        .to_le_bytes(),
                );
                digest.update([match projection.algebra.kind {
                    semantic_vocabulary::ContentAlgebraKind::IntervalSet => 1,
                    semantic_vocabulary::ContentAlgebraKind::CountedQuantity => 2,
                }]);
                hash_bytes(digest, projection.algebra.parameter.as_bytes());
            }
        }
    }
}

fn hash_internal_scalar_argument_source(
    digest: &mut Sha256,
    source: &machine_code::InternalUnitScalarArgumentSourceRecord,
) {
    match source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            source_value,
            instruction,
            ..
        } => {
            digest.update([6]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
            source_value,
            instruction,
            ..
        } => {
            digest.update([5]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
            source_value,
            instruction,
            scratch_byte_offset,
            ..
        } => {
            digest.update([4]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
            digest.update(scratch_byte_offset.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            ..
        } => {
            digest.update([0]);
            digest.update(parameter_index.to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            value,
            ..
        } => {
            digest.update([1]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    digest.update(value.to_le_bytes())
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    digest.update(value.to_le_bytes())
                }
            }
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            digest.update([3]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            digest.update([u8::from(*value)]);
            digest.update(
                u64::try_from(*definition_ordinal)
                    .expect("validated definition ordinal is u64-representable")
                    .to_le_bytes(),
            );
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            digest.update([2]);
            digest.update(home.defining_operation.get().to_le_bytes());
            digest.update(home.source_value.get().to_le_bytes());
            digest.update(home.byte_offset.to_le_bytes());
        }
    }
}

pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(crate) fn hash_target(digest: &mut Sha256, target: NativeTarget) {
    digest.update([match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    }]);
    digest.update(canonical_usize(target.pointer_size));
    digest.update(canonical_usize(target.pointer_alignment));
}

pub(crate) fn hash_object_symbol(digest: &mut Sha256, symbol: object_file::ObjectSymbolHandle) {
    digest.update([u8::from(symbol.is_valid())]);
    digest.update(u64::from(symbol.arena_index()).to_le_bytes());
    digest.update(u64::from(symbol.generation()).to_le_bytes());
}

pub(crate) fn hash_machine_function_identity(
    digest: &mut Sha256,
    identity: function_identity::MachineFunctionIdentity,
) {
    let (tag, continuation, coordinate) = if let Some(source) = identity.source_key() {
        (1, source, 0)
    } else if let Some(continuation) = identity.program_storage_entry_continuation() {
        (2, continuation, 0)
    } else {
        (
            3,
            identity.associated_source_continuation(),
            identity
                .callback_thunk_placement_index()
                .expect("machine function identity has one closed role"),
        )
    };
    digest.update([tag]);
    for symbol in [continuation.machine, continuation.state] {
        digest.update([u8::from(symbol.is_valid())]);
        digest.update(u64::from(symbol.arena_index()).to_le_bytes());
        digest.update(u64::from(symbol.generation()).to_le_bytes());
    }
    digest.update(canonical_usize(continuation.segment_index));
    digest.update(canonical_usize(coordinate));
}

pub(crate) fn hash_callback_relocation(
    digest: &mut Sha256,
    relocation: NormalizedForeignCallbackRelocation,
) {
    hash_object_symbol(digest, relocation.object_symbol());
    hash_relocation_origin(digest, relocation.origin());
    digest.update(canonical_usize(relocation.offset()));
    digest.update(canonical_usize(relocation.byte_width()));
    digest.update(relocation.addend().to_le_bytes());
    digest.update([relocation_kind_tag(relocation.kind())]);
}

pub(crate) fn hash_relocation_origin(digest: &mut Sha256, origin: RelocationOrigin) {
    hash_object_symbol(digest, origin.symbol_handle());
    let (tag, coordinate) = match origin {
        RelocationOrigin::Instruction {
            selected_instruction_index,
            ..
        } => (1, u64::from(selected_instruction_index)),
        RelocationOrigin::SemanticOperation {
            operation_identity, ..
        } => (2, operation_identity),
        RelocationOrigin::SemanticEdge { edge_identity, .. } => (3, edge_identity),
        RelocationOrigin::Materialization { .. } => (4, 0),
    };
    digest.update([tag]);
    digest.update(coordinate.to_le_bytes());
}

pub(crate) const fn relocation_kind_tag(kind: RelocationKind) -> u8 {
    match kind {
        RelocationKind::Aarch64Page21 => 1,
        RelocationKind::Aarch64PageOffset12 => 2,
        RelocationKind::Aarch64Branch26 => 3,
        RelocationKind::Absolute64 => 4,
        RelocationKind::X86_64Relative32 => 5,
    }
}

pub(crate) fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(canonical_usize(bytes.len()));
    digest.update(bytes);
}

pub(crate) fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("native physical evidence field fits u64")
        .to_le_bytes()
}

/// Canonical identity of the one subject that stopped a scoped derivation.
/// Occurrence subjects hash their exact occurrence identity (which already
/// commits terminal, machine, operation, boundary, and ordinal); record and
/// machine subjects hash their retained coordinates directly.
pub(crate) fn physical_evidence_gap_identity(
    subject: &NativePhysicalEvidenceGapSubject,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical-evidence-gap.sha256.v1\0");
    match subject {
        NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner { machine, owner } => {
            digest.update([2]);
            digest.update(machine.get().to_le_bytes());
            match owner {
                CallSiteOwner::Operation(operation) => {
                    digest.update([1]);
                    digest.update(operation.get().to_le_bytes());
                    digest.update(0_u32.to_le_bytes());
                }
                CallSiteOwner::CleanupAction {
                    edge,
                    action_ordinal,
                } => {
                    digest.update([2]);
                    digest.update(edge.get().to_le_bytes());
                    digest.update(action_ordinal.to_le_bytes());
                }
            }
        }
        NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization { occurrence } => {
            digest.update([3]);
            digest.update(occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnsupportedNormalizedForeignCall { occurrence } => {
            digest.update([4]);
            digest.update(occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence { occurrence } => {
            digest.update([5]);
            digest.update(occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan { occurrence } => {
            digest.update([6]);
            digest.update(occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnownedPortEffect {
            machine,
            psi_operation,
            service,
            port,
            value,
            operation_ordinal,
            code_offset,
            byte_count,
        } => {
            digest.update([7]);
            digest.update(machine.get().to_le_bytes());
            hash_port_effect_record(
                &mut digest,
                &PortEffectRecord {
                    psi_operation: *psi_operation,
                    service: *service,
                    port: *port,
                    value: *value,
                    operation_ordinal: *operation_ordinal,
                    code_offset: *code_offset,
                    byte_count: *byte_count,
                },
            );
        }
    }
    digest.finalize().into()
}
