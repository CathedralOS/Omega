/*
We provide the canonical SHA-256 domain-digest encodings for structural paths,
sum layouts, port effects, boundary settlement records, ABI value placements,
relocations, and evidence gaps across physical evidence derivation.
*/

pub(crate) use crate::NormalizedForeignCallbackRelocation;
use machine_code::{BoundaryExecutionRecord, PortEffectRecord};
use object_file::{RelocationOrigin, relocation_kind_tag};
use semantic_vocabulary::{IntegerSign, ScalarType};
use target::NativeTarget;
use target_vocabulary::{
    BoundaryRealization, CallSiteOwner, CompilerBuiltinExecution, CompletionClaimSource,
};

use crate::physical::NativePhysicalEvidenceGapSubject;

pub(crate) fn hash_structural_path(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    path: &[terminal_psi::StructuralPathSegment],
) {
    digest.u64(path.len() as u64);
    for segment in path {
        match segment {
            terminal_psi::StructuralPathSegment::AllElements => digest.byte(10),
            terminal_psi::StructuralPathSegment::RepresentationView {
                structural_type,
                stored_scalar,
            } => {
                digest.byte(8);
                digest.raw(&structural_type.get().to_le_bytes());
                digest.byte(u8::from(stored_scalar.is_some()));
                if let Some(scalar) = stored_scalar {
                    hash_scalar_type(digest, *scalar);
                }
            }
            terminal_psi::StructuralPathSegment::Case(case) => {
                digest.byte(7);
                digest.raw(&case.get().to_le_bytes());
            }
            terminal_psi::StructuralPathSegment::Referent => digest.byte(3),
            terminal_psi::StructuralPathSegment::FixedRange { start, end } => {
                digest.byte(4);
                digest.raw(&start.to_le_bytes());
                digest.raw(&end.to_le_bytes());
            }
            terminal_psi::StructuralPathSegment::Field(identity) => {
                digest.byte(1);
                digest.raw(&identity.0.to_le_bytes());
            }
            terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                digest.byte(2);
                digest.raw(&index.to_le_bytes());
            }
            terminal_psi::StructuralPathSegment::RuntimeIndex { index, obligation } => {
                digest.byte(5);
                digest.raw(&index.get().to_le_bytes());
                digest.raw(&obligation.get().to_le_bytes());
            }
        }
    }
}

pub(crate) fn hash_sum_layout(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    layout: &calling_conventions::ConventionalSumLayout,
) {
    hash_integer_shape(digest, layout.shape);
    digest.raw(&layout.tag_byte_offset.to_le_bytes());
    hash_integer_shape(digest, layout.tag_shape);
    hash_packed_fields(digest, &layout.common_fields);
    digest.raw(&layout.payload_byte_offset.to_le_bytes());
    digest.u64(layout.cases.len() as u64);
    for case in &layout.cases {
        hash_packed_fields(digest, &case.fields);
    }
}

fn hash_packed_fields(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    fields: &[calling_conventions::PackedFieldLayout],
) {
    digest.u64(fields.len() as u64);
    for field in fields {
        digest.raw(&field.byte_offset.to_le_bytes());
        hash_integer_shape(digest, field.shape);
    }
}

fn hash_integer_shape(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    shape: calling_conventions::ValueShape,
) {
    debug_assert_eq!(shape.class, calling_conventions::ValueClass::Integer);
    digest.byte(1);
    digest.raw(&shape.byte_size.to_le_bytes());
    digest.raw(&shape.alignment.to_le_bytes());
}

pub(crate) fn hash_port_effect_record(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    effect: &PortEffectRecord,
) {
    digest.raw(&effect.psi_operation.get().to_le_bytes());
    digest.raw(&effect.service.get().to_le_bytes());
    digest.raw(&effect.port.to_le_bytes());
    digest.byte(effect.value);
    digest.len(effect.operation_ordinal);
    digest.len(effect.code_offset);
    digest.len(effect.byte_count);
}

pub(crate) fn hash_boundary_settlement_record(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    settlement: &machine_code::BoundarySettlementRecord,
) -> Result<(), &'static str> {
    hash_invocation_site(digest, settlement.invocation_site);
    digest.raw(&settlement.boundary.get().to_le_bytes());
    match settlement.execution {
        BoundaryExecutionRecord::NativeBinding(native) => {
            digest.byte(3);
            digest.raw(&native.receipt());
            digest.raw(&native.contract());
            digest.raw(&native.calling());
        }
        BoundaryExecutionRecord::AdmittedProvider(record) => {
            digest.byte(1);
            hash_provider_execution_record(digest, &record);
        }
        BoundaryExecutionRecord::CompilerBuiltin(execution) => {
            digest.byte(2);
            digest.byte(compiler_builtin_execution_tag(execution));
            if matches!(
                execution,
                CompilerBuiltinExecution::AccessorScalarAccess { .. }
                    | CompilerBuiltinExecution::AccessLifecycle { .. }
                    | CompilerBuiltinExecution::StaticInterrupt { .. }
            ) {
                let mut bytes = Vec::new();
                target_vocabulary::identity::encode_boundary_execution_identity(
                    &mut bytes,
                    target_vocabulary::BoundaryExecutionBinding::CompilerBuiltin(execution),
                );
                digest.raw(&bytes);
            }
        }
    }
    hash_boundary_realization(digest, &settlement.realization);
    digest.len(settlement.scalar_arguments.len());
    for argument in &settlement.scalar_arguments {
        digest.raw(&argument.source_value.get().to_le_bytes());
        hash_scalar_type(digest, argument.scalar_type);
        hash_integer_value(digest, argument.immediate);
        hash_machine_register(digest, argument.destination);
    }
    digest.len(settlement.runtime_scalar_arguments.len());
    for argument in &settlement.runtime_scalar_arguments {
        digest.raw(&argument.parameter_index.to_le_bytes());
        hash_internal_scalar_argument_source(digest, &argument.source);
        hash_value_placement(digest, &argument.placement);
        digest.len(argument.code_offset);
        digest.len(argument.byte_count);
    }
    digest.len(settlement.arguments.len());
    for argument in &settlement.arguments {
        hash_structural_argument(digest, argument);
    }
    digest.len(settlement.byte_sequence_arguments.len());
    for custody in &settlement.byte_sequence_arguments {
        hash_structural_argument(digest, &custody.argument);
        digest.raw(&custody.literal_operation.get().to_le_bytes());
        let declaration = terminal_codec::encode_structural_type_declaration(
            &custody.structural_type,
        )
        .map_err(|_| "installed D41 byte-sequence declaration cannot be encoded canonically")?;
        digest.bytes(&declaration);
        digest.bytes(&custody.bytes);
        digest.len(custody.code_offset);
        digest.len(custody.code_byte_count);
        digest.len(custody.data_offset);
        digest.len(custody.data_byte_count);
    }
    digest.len(settlement.completion_claim_sources.len());
    for source in &settlement.completion_claim_sources {
        hash_claim_source(digest, source);
    }
    digest.len(settlement.completion_receipts.len());
    for receipt in &settlement.completion_receipts {
        digest.raw(&receipt.claim.get().to_le_bytes());
        digest.raw(&receipt.argument_index.to_le_bytes());
        hash_structural_path(digest, &receipt.path);
    }
    digest.len(settlement.completion_provider_custody.len());
    for binding in &settlement.completion_provider_custody {
        hash_claim_source(digest, &binding.source);
        digest.raw(&binding.receipt.claim.get().to_le_bytes());
        digest.raw(&binding.receipt.argument_index.to_le_bytes());
        hash_provider_execution_record(digest, &binding.provider_execution);
    }
    hash_boundary_result_record(digest, &settlement.native_result)?;
    digest.len(settlement.operation_ordinal);
    digest.len(settlement.code_offset);
    digest.len(settlement.byte_count);
    Ok(())
}

const fn compiler_builtin_execution_tag(execution: CompilerBuiltinExecution) -> u8 {
    match execution {
        CompilerBuiltinExecution::StaticInterrupt { .. } => 254,
        CompilerBuiltinExecution::AccessLifecycle { .. } => 253,
        CompilerBuiltinExecution::EmptyRunRelease => 249,
        CompilerBuiltinExecution::AccessorScalarAccess { .. } => 250,
        CompilerBuiltinExecution::ProgramImageRetainedSuffixStart => 251,
        CompilerBuiltinExecution::ProgramStorageEntry => 252,
        CompilerBuiltinExecution::HostedExitProcessI32 => 1,
        CompilerBuiltinExecution::HostedReadByte => 2,
        CompilerBuiltinExecution::HostedWriteByteI32 => 3,
        CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } => {
            4 + 2 * float_arithmetic_tag(operation) + float_format_tag(format)
        }
    }
}

const fn float_arithmetic_tag(operation: target_vocabulary::IeeeFloatArithmeticOperation) -> u8 {
    operation.ordinal()
}

const fn float_format_tag(format: semantic_vocabulary::IeeeFloatFormat) -> u8 {
    match format {
        semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
        semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
    }
}

fn hash_boundary_realization(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    realization: &BoundaryRealization,
) {
    match realization {
        BoundaryRealization::StaticInterrupt {
            boundary,
            operation,
        } => {
            // Tag 12 is `EmptyRunRelease`; this matches the target-vocabulary encoder.
            digest.byte(13);
            digest.raw(&boundary.get().to_le_bytes());
            digest.byte(operation.code());
        }
        BoundaryRealization::AccessLifecycle { boundary } => {
            digest.byte(11);
            digest.raw(&boundary.get().to_le_bytes());
        }
        BoundaryRealization::EmptyRunRelease => digest.byte(12),
        BoundaryRealization::ProgramImageRetainedSuffixStart => digest.byte(10),
        BoundaryRealization::AccessorScalarAccess(access) => {
            digest.byte(9);
            digest.raw(&access.contract);
            let mut bytes = Vec::new();
            target_vocabulary::identity::encode_accessor_scalar_demand(&mut bytes, access.demand);
            digest.raw(&bytes);
        }
        BoundaryRealization::MetadataOnlyPort(realization) => {
            digest.byte(1);
            digest.raw(&realization.effect_operation.get().to_le_bytes());
            digest.raw(&realization.service.get().to_le_bytes());
            digest.raw(&realization.port.to_le_bytes());
            digest.byte(realization.value);
        }
        BoundaryRealization::DirectPortReadU8(realization) => {
            digest.byte(2);
            digest.raw(&realization.service.get().to_le_bytes());
            digest.raw(&realization.port.to_le_bytes());
        }
        BoundaryRealization::LinuxWriteLine(_) => digest.byte(3),
        BoundaryRealization::ClaimCompletionOnly(_) => digest.byte(4),
        BoundaryRealization::HostedExitProcessI32(_) => digest.byte(5),
        BoundaryRealization::HostedReadByte(_) => digest.byte(6),
        BoundaryRealization::HostedWriteByteI32(_) => digest.byte(7),
        BoundaryRealization::NearestIeeeFloatArithmetic(realization) => {
            digest.byte(8);
            digest.byte(float_arithmetic_tag(realization.operation));
            digest.byte(float_format_tag(realization.format));
            if let Some(provider) = realization.x86_scalar_fma {
                digest.raw(&provider.identity());
            }
        }
    }
}

fn hash_boundary_result_record(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    result: &machine_code::BoundaryResultRecord,
) -> Result<(), &'static str> {
    match result {
        machine_code::BoundaryResultRecord::Unit => digest.byte(0),
        machine_code::BoundaryResultRecord::Scalar(result) => {
            digest.byte(1);
            digest.raw(&result.value.get().to_le_bytes());
            hash_scalar_type(digest, result.scalar_type);
            hash_value_placement(digest, &result.placement);
            digest.raw(&result.return_edge.get().to_le_bytes());
        }
        machine_code::BoundaryResultRecord::Structural(result) => {
            digest.byte(2);
            hash_boundary_structural_result(digest, result).map_err(
                |_| "boundary structural result declaration cannot be encoded canonically",
            )?;
        }
    }
    Ok(())
}

pub(super) fn hash_boundary_structural_result(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    result: &machine_code::BoundaryStructuralResultRecord,
) -> Result<(), terminal_codec::CodecError> {
    digest.raw(&result.defining_operation.get().to_le_bytes());
    digest.raw(&result.result.place.get().to_le_bytes());
    digest.raw(&result.result.structural_type.get().to_le_bytes());
    digest.byte(structural_multiplicity_tag(result.result.multiplicity));
    hash_qualifications(
        digest,
        &result.result.qualifications,
        &result.result.projected_qualifications,
    );
    digest.u64(result.result.claims.len() as u64);
    for claim in &result.result.claims {
        digest.raw(&claim.claim.get().to_le_bytes());
        hash_structural_path(digest, &claim.path);
    }
    let declaration = terminal_codec::encode_structural_type_declaration(&result.declaration)?;
    digest.bytes(&declaration);
    hash_sum_layout(digest, &result.layout);
    digest.raw(&result.home_byte_offset.to_le_bytes());
    Ok(())
}

fn hash_scalar_type(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    scalar_type: ScalarType,
) {
    match scalar_type {
        ScalarType::Callable(contract) => {
            digest.byte(4);
            digest.raw(&contract.get().to_le_bytes());
        }
        ScalarType::Boolean => digest.byte(1),
        ScalarType::Integer(integer) => {
            digest.byte(2);
            digest.byte(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            integer.width().write_identity(|part| digest.raw(part));
            digest.byte(match integer.carrier() {
                semantic_vocabulary::IntegerCarrier::Fixed => 1,
                semantic_vocabulary::IntegerCarrier::Address => 2,
            });
        }
        ScalarType::IeeeFloat(format) => {
            digest.byte(3);
            digest.byte(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 1,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 2,
            });
        }
    }
}

fn hash_integer_value(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    value: semantic_vocabulary::IntegerValue,
) {
    match value {
        semantic_vocabulary::IntegerValue::Signed(value) => {
            digest.byte(1);
            digest.raw(&value.to_le_bytes());
        }
        semantic_vocabulary::IntegerValue::Unsigned(value) => {
            digest.byte(2);
            digest.raw(&value.to_le_bytes());
        }
    }
}

fn hash_machine_register(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    register: calling_conventions::MachineRegister,
) {
    pub(crate) use calling_conventions::MachineRegister;
    match register {
        MachineRegister::X86Rax => digest.byte(1),
        MachineRegister::X86Rcx => digest.byte(2),
        MachineRegister::X86Rdx => digest.byte(3),
        MachineRegister::X86Rbx => digest.byte(4),
        MachineRegister::X86Rsp => digest.byte(5),
        MachineRegister::X86Rbp => digest.byte(6),
        MachineRegister::X86Rsi => digest.byte(7),
        MachineRegister::X86Rdi => digest.byte(8),
        MachineRegister::X86R8 => digest.byte(9),
        MachineRegister::X86R9 => digest.byte(10),
        MachineRegister::X86R10 => digest.byte(11),
        MachineRegister::X86R11 => digest.byte(12),
        MachineRegister::X86R12 => digest.byte(13),
        MachineRegister::X86R13 => digest.byte(14),
        MachineRegister::X86R14 => digest.byte(15),
        MachineRegister::X86R15 => digest.byte(16),
        MachineRegister::X86Xmm(index) => {
            digest.byte(17);
            digest.byte(index);
        }
        MachineRegister::Aarch64X(index) => {
            digest.byte(18);
            digest.byte(index);
        }
        MachineRegister::Aarch64V(index) => {
            digest.byte(19);
            digest.byte(index);
        }
    }
}

fn hash_value_placement(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    placement: &calling_conventions::ValuePlacement,
) {
    hash_value_shape(digest, placement.shape);
    digest.len(placement.locations.len());
    for location in &placement.locations {
        match location {
            calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } => {
                digest.byte(1);
                hash_machine_register(digest, *register);
                digest.raw(&value_byte_offset.to_le_bytes());
                digest.raw(&byte_size.to_le_bytes());
            }
            calling_conventions::ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset,
                byte_size,
                alignment,
            } => {
                digest.byte(2);
                digest.raw(&stack_byte_offset.to_le_bytes());
                digest.raw(&value_byte_offset.to_le_bytes());
                digest.raw(&byte_size.to_le_bytes());
                digest.raw(&alignment.to_le_bytes());
            }
            calling_conventions::ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            } => {
                digest.byte(3);
                match pointer {
                    calling_conventions::IndirectPointerLocation::Register(register) => {
                        digest.byte(1);
                        hash_machine_register(digest, *register);
                    }
                    calling_conventions::IndirectPointerLocation::Stack {
                        stack_byte_offset,
                        alignment,
                    } => {
                        digest.byte(2);
                        digest.raw(&stack_byte_offset.to_le_bytes());
                        digest.raw(&alignment.to_le_bytes());
                    }
                }
                match copy_stack_byte_offset {
                    None => digest.byte(0),
                    Some(offset) => {
                        digest.byte(1);
                        digest.raw(&offset.to_le_bytes());
                    }
                }
                digest.raw(&byte_size.to_le_bytes());
                digest.raw(&alignment.to_le_bytes());
            }
        }
    }
}

fn hash_value_shape(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    shape: calling_conventions::ValueShape,
) {
    match shape.class {
        calling_conventions::ValueClass::Integer => digest.byte(1),
        calling_conventions::ValueClass::IntegerAggregate => digest.byte(7),
        calling_conventions::ValueClass::Float => digest.byte(2),
        calling_conventions::ValueClass::BorrowedReference => digest.byte(3),
        calling_conventions::ValueClass::BorrowedScalarReference => digest.byte(6),
        calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
            digest.byte(4);
            digest.byte(members);
        }
        calling_conventions::ValueClass::SystemVAggregate { first, second } => {
            digest.byte(5);
            for class in [first, second] {
                digest.byte(match class {
                    calling_conventions::SystemVEightbyteClass::Integer => 1,
                    calling_conventions::SystemVEightbyteClass::Sse => 2,
                });
            }
        }
    }
    digest.raw(&shape.byte_size.to_le_bytes());
    digest.raw(&shape.alignment.to_le_bytes());
}

pub(crate) fn hash_structural_argument(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    argument: &terminal_psi::StructuralArgument,
) {
    digest.raw(&argument.place.get().to_le_bytes());
    digest.byte(structural_access_tag(argument.access));
    hash_structural_path(digest, &argument.path);
}

pub(crate) fn hash_structural_parameter_declaration(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    parameter: &terminal_psi::StructuralParameterDeclaration,
) {
    digest.raw(&parameter.place.get().to_le_bytes());
    digest.raw(&parameter.position.to_le_bytes());
    digest.raw(&parameter.structural_type.get().to_le_bytes());
    digest.byte(structural_multiplicity_tag(parameter.multiplicity));
    digest.byte(structural_access_tag(parameter.access));
    hash_qualifications(
        digest,
        &parameter.qualifications,
        &parameter.projected_qualifications,
    );
}

fn structural_multiplicity_tag(multiplicity: terminal_psi::StructuralMultiplicity) -> u8 {
    match multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    }
}

fn structural_access_tag(access: terminal_psi::StructuralAccess) -> u8 {
    match access {
        terminal_psi::StructuralAccess::Owned => 1,
        terminal_psi::StructuralAccess::SharedBorrow => 2,
        terminal_psi::StructuralAccess::MutableBorrow => 3,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => 4,
    }
}

fn hash_qualifications(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    qualifications: &[semantic_vocabulary::StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
) {
    digest.len(qualifications.len());
    for domain in qualifications {
        digest.raw(&domain.get().to_le_bytes());
    }
    digest.len(projected.len());
    for qualification in projected {
        hash_structural_path(digest, &qualification.path);
        digest.raw(&qualification.domain.get().to_le_bytes());
    }
}

pub(super) fn hash_claim_source(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    source: &CompletionClaimSource,
) {
    digest.raw(&source.claim.get().to_le_bytes());
    match &source.entry {
        None => digest.byte(0),
        Some(entry) => {
            digest.byte(1);
            digest.raw(&entry.claim.get().to_le_bytes());
            digest.raw(&entry.input.get().to_le_bytes());
            hash_structural_path(digest, &entry.path);
        }
    }
    match &source.content {
        None => digest.byte(0),
        Some(content) => {
            digest.byte(1);
            digest.raw(&content.claim.get().to_le_bytes());
            digest.byte(match content.input.version {
                semantic_vocabulary::ContentPlaceVersion::Entry => 1,
                semantic_vocabulary::ContentPlaceVersion::Current => 2,
            });
            digest.raw(&content.input.root.get().to_le_bytes());
            digest.len(content.input.segments.len());
            for segment in &content.input.segments {
                match segment {
                    semantic_vocabulary::ContentPlaceSegment::Case(identity) => {
                        digest.byte(1);
                        digest.bytes(identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::Field(identity) => {
                        digest.byte(2);
                        digest.bytes(identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::FixedIndex(index) => {
                        digest.byte(3);
                        digest.raw(&index.to_le_bytes());
                    }
                }
            }
            digest.len(content.projections.len());
            for projection in &content.projections {
                digest.raw(&projection.projection.domain.get().to_le_bytes());
                digest.raw(
                    &projection
                        .projection
                        .projection_report_fingerprint
                        .to_le_bytes(),
                );
                digest.byte(match projection.algebra.kind {
                    semantic_vocabulary::ContentAlgebraKind::IntervalSet => 1,
                    semantic_vocabulary::ContentAlgebraKind::CountedQuantity => 2,
                });
                digest.bytes(projection.algebra.parameter.as_bytes());
            }
        }
    }
}

pub(super) fn hash_internal_scalar_argument_source(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    source: &machine_code::InternalUnitScalarArgumentSourceRecord,
) {
    match source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            source_value,
            instruction,
            ..
        } => {
            digest.byte(6);
            digest.raw(&source_value.get().to_le_bytes());
            digest.raw(&instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
            source_value,
            instruction,
            ..
        } => {
            digest.byte(5);
            digest.raw(&source_value.get().to_le_bytes());
            digest.raw(&instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
            source_value,
            instruction,
            scratch_byte_offset,
            ..
        } => {
            digest.byte(4);
            digest.raw(&source_value.get().to_le_bytes());
            digest.raw(&instruction.0.to_le_bytes());
            digest.raw(&scratch_byte_offset.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            ..
        } => {
            digest.byte(0);
            digest.raw(&parameter_index.to_le_bytes());
            digest.raw(&source_value.get().to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            value,
            ..
        } => {
            digest.byte(1);
            digest.raw(&defining_operation.get().to_le_bytes());
            digest.raw(&source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    digest.raw(&value.to_le_bytes())
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    digest.raw(&value.to_le_bytes())
                }
            }
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            digest.byte(3);
            digest.raw(&defining_operation.get().to_le_bytes());
            digest.raw(&source_value.get().to_le_bytes());
            digest.byte(u8::from(*value));
            digest.raw(
                &u64::try_from(*definition_ordinal)
                    .expect("validated definition ordinal is u64-representable")
                    .to_le_bytes(),
            );
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            digest.byte(2);
            digest.raw(&home.defining_operation.get().to_le_bytes());
            digest.raw(&home.source_value.get().to_le_bytes());
            digest.raw(&home.byte_offset.to_le_bytes());
        }
    }
}

pub(crate) fn hash_target(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    target: NativeTarget,
) {
    digest.raw(&target.wire_identity_bytes());
}

pub(crate) fn hash_object_symbol(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    symbol: object_file::ObjectSymbolHandle,
) {
    digest.byte(u8::from(symbol.is_valid()));
    digest.u64(u64::from(symbol.arena_index()));
    digest.u64(u64::from(symbol.generation()));
}

pub(crate) fn hash_machine_function_identity(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    identity: function_identity::MachineFunctionIdentity,
) {
    digest.raw(&identity.to_bytes());
}

pub(crate) fn hash_callback_relocation(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    relocation: NormalizedForeignCallbackRelocation,
) {
    hash_object_symbol(digest, relocation.object_symbol());
    hash_relocation_origin(digest, relocation.origin());
    digest.len(relocation.offset());
    digest.len(relocation.byte_width());
    digest.raw(&relocation.addend().to_le_bytes());
    digest.byte(relocation_kind_tag(relocation.kind()));
}

pub(crate) fn hash_relocation_origin(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    origin: RelocationOrigin,
) {
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
    digest.byte(tag);
    digest.raw(&coordinate.to_le_bytes());
}

pub(crate) fn hash_call_site_owner(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    owner: &CallSiteOwner,
) {
    match owner {
        CallSiteOwner::Operation(operation) => {
            digest.byte(1);
            digest.raw(&operation.get().to_le_bytes());
            digest.raw(&0_u32.to_le_bytes());
        }
        CallSiteOwner::InvokeNoNormal(edge) => {
            digest.byte(4);
            digest.raw(&edge.get().to_le_bytes());
            digest.raw(&0_u32.to_le_bytes());
        }
        CallSiteOwner::ExternalCompletion(edge) => {
            digest.byte(3);
            digest.raw(&edge.get().to_le_bytes());
            digest.raw(&0_u32.to_le_bytes());
        }
        CallSiteOwner::Installation { operation, role } => {
            digest.byte(5);
            digest.raw(&operation.get().to_le_bytes());
            digest.byte(match role {
                target_vocabulary::InstallationCallRole::Validate => 0,
                target_vocabulary::InstallationCallRole::Work => 1,
            });
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            digest.byte(2);
            digest.raw(&edge.get().to_le_bytes());
            digest.raw(&action_ordinal.to_le_bytes());
        }
    }
}

pub(crate) fn physical_evidence_gap_identity(
    subject: &NativePhysicalEvidenceGapSubject,
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-physical-evidence-gap.sha256.v1\0",
    );
    match subject {
        NativePhysicalEvidenceGapSubject::UncoveredPrivateFunction { function } => {
            digest.byte(8);
            hash_machine_function_identity(&mut digest, *function);
        }
        NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner { machine, owner } => {
            digest.byte(2);
            digest.raw(&machine.get().to_le_bytes());
            hash_call_site_owner(&mut digest, owner);
        }
        NativePhysicalEvidenceGapSubject::UnsupportedSettlementRealization { occurrence } => {
            digest.byte(3);
            digest.raw(&occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnsupportedNormalizedForeignCall { occurrence } => {
            digest.byte(4);
            digest.raw(&occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnrealizedBoundaryOccurrence { occurrence } => {
            digest.byte(5);
            digest.raw(&occurrence.identity().bytes());
        }
        NativePhysicalEvidenceGapSubject::UnsupportedOperatorSpan { occurrence } => {
            digest.byte(6);
            digest.raw(&occurrence.identity().bytes());
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
            digest.byte(7);
            digest.raw(&machine.get().to_le_bytes());
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
    digest.finish()
}

#[cfg(test)]
mod width_tests {
    use super::*;
    use semantic_vocabulary::{IntegerType, MachineId, ValueId, WidthParameter};

    fn scalar_identity(integer: IntegerType) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(b"native-scalar-test");
        hash_scalar_type(&mut digest, ScalarType::Integer(integer));
        digest.finish()
    }

    #[test]
    fn native_scalar_hash_preserves_legacy_bytes_and_rigid_width_scope() {
        let fixed = IntegerType::new(IntegerSign::Unsigned, 13).unwrap();
        let mut legacy = crate::digest_encoder::DomainDigestEncoder::new(b"native-scalar-test");
        legacy.raw(&[2, 2, 13, 0, 1]);
        assert_eq!(scalar_identity(fixed), legacy.finish());
        let parameter = |owner, value| {
            IntegerType::parameterized(
                IntegerSign::Unsigned,
                WidthParameter {
                    owner: MachineId::new(owner).unwrap(),
                    value: ValueId::new(value).unwrap(),
                },
            )
        };
        assert_ne!(
            scalar_identity(parameter(1, 2)),
            scalar_identity(parameter(2, 2))
        );
        assert_ne!(
            scalar_identity(parameter(1, 2)),
            scalar_identity(parameter(1, 3))
        );
        assert_ne!(scalar_identity(parameter(1, 2)), scalar_identity(fixed));
    }
}

#[cfg(test)]
mod tag_tests {
    use super::*;
    use language_core::static_interrupts::StaticInterruptOperation;
    use semantic_vocabulary::BoundaryMachineId;

    // A static interrupt and an empty-run release are different settlements.
    // Each needs its own leading tag, or the interrupt's trailing fields can
    // be read as the release followed by the next record field.
    #[test]
    fn static_interrupt_and_empty_run_release_keep_distinct_tags() {
        let boundary = BoundaryMachineId::new(9).expect("boundary");
        let operation = StaticInterruptOperation::Install;

        assert_ne!(
            compiler_builtin_execution_tag(CompilerBuiltinExecution::StaticInterrupt {
                boundary,
                operation,
            }),
            compiler_builtin_execution_tag(CompilerBuiltinExecution::EmptyRunRelease)
        );

        let mut interrupt =
            crate::digest_encoder::DomainDigestEncoder::new(b"realization-tag-test");
        hash_boundary_realization(
            &mut interrupt,
            &BoundaryRealization::StaticInterrupt {
                boundary,
                operation,
            },
        );
        let mut release_then_fields =
            crate::digest_encoder::DomainDigestEncoder::new(b"realization-tag-test");
        hash_boundary_realization(
            &mut release_then_fields,
            &BoundaryRealization::EmptyRunRelease,
        );
        release_then_fields.raw(&boundary.get().to_le_bytes());
        release_then_fields.byte(operation.code());
        assert_ne!(interrupt.finish(), release_then_fields.finish());
    }
}

/// Hashes the selected normalized-foreign mechanism. Import and Component
/// mechanisms return their same-stack contribution so each caller can append
/// the contribution fields its own identity covers.
pub(crate) fn hash_normalized_foreign_mechanism<'a>(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    mechanism: &'a target_vocabulary::NormalizedForeignCallMechanism,
) -> Option<&'a provider_stack_contribution::AdmittedSameStackContribution> {
    use target_vocabulary::NormalizedForeignCallMechanism as Mechanism;
    match mechanism {
        Mechanism::CompilerImport {
            import,
            requirement_identity,
            selected_plan,
        } => {
            digest.byte(6);
            digest.byte(import.tag());
            digest.raw(selected_plan);
            digest.bytes(requirement_identity.as_bytes());
            None
        }
        Mechanism::BuildImport {
            stack,
            locator,
            receipt,
            requirement_identity,
        } => {
            digest.byte(5);
            digest.raw(&locator.identity_digest().as_bytes());
            digest.raw(&stack.commitment());
            digest.raw(receipt);
            digest.bytes(requirement_identity.as_bytes());
            None
        }
        Mechanism::Table {
            target,
            byte_offset,
            table_operand,
            dispatch_only,
            source,
        } => {
            digest.byte(3);
            digest.bytes(target.target_name().as_bytes());
            digest.raw(&byte_offset.to_le_bytes());
            digest.raw(&table_operand.to_le_bytes());
            digest.byte(u8::from(*dispatch_only));
            digest.raw(&source.commitment());
            None
        }
        Mechanism::Import {
            locator,
            same_stack_contribution,
        } => {
            digest.byte(1);
            digest.raw(&locator.identity_digest().as_bytes());
            Some(same_stack_contribution)
        }
        Mechanism::Component {
            locator,
            provider,
            candidate,
            application_commitment,
            same_stack_contribution,
        } => {
            digest.byte(4);
            digest.raw(locator.identity_digest());
            digest.raw(&provider.vocabulary_marker.get().to_le_bytes());
            digest.raw(provider.program_fingerprint.as_bytes());
            digest.raw(&candidate.get().to_le_bytes());
            digest.raw(application_commitment);
            Some(same_stack_contribution)
        }
        Mechanism::SystemCall(kernel) => {
            digest.byte(2);
            digest.bytes(kernel.target.target_name().as_bytes());
            digest.raw(&kernel.number.to_le_bytes());
            match kernel.execution {
                target_vocabulary::NormalizedForeignExecution::ProviderPlan {
                    report,
                    commitment,
                } => {
                    digest.byte(0);
                    digest.raw(&report.to_le_bytes());
                    digest.raw(&commitment);
                }
                target_vocabulary::NormalizedForeignExecution::NativeBinding(binding) => {
                    digest.byte(1);
                    digest.raw(&binding);
                }
            }
            digest.bytes(kernel.requirement_identity.as_bytes());
            None
        }
    }
}

/// Hashes the five provider report coordinates in their canonical order.
pub(crate) fn hash_provider_execution_record(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    record: &machine_code::ProviderExecutionRecord,
) {
    for coordinate in [
        record.provider_plan_report_identity,
        record.provider_execution_report_identity,
        record.provider_execution_report_fingerprint,
        record.normalized_root_report_identity,
        record.boundary_contract_report_fingerprint,
    ] {
        digest.u64(coordinate);
    }
}

pub(crate) fn hash_invocation_site(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    site: semantic_vocabulary::InvocationSite,
) {
    let mut canonical = Vec::with_capacity(9);
    crate::physical::identity_bytes::append_invocation_site(&mut canonical, site);
    digest.raw(&canonical);
}
