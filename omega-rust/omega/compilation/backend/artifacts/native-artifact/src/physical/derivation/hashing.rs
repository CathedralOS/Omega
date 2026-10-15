//! The SHA-256 encodings every identity in the derivation is built from.

pub(crate) use crate::NormalizedForeignCallbackRelocation;
use machine_code::{BoundaryExecutionRecord, PortEffectRecord};
use object_file::{RelocationOrigin, relocation_kind_tag};
use semantic_vocabulary::{IntegerSign, ScalarType};
use sha2::Digest;
use sha2::Sha256;
use target::{Architecture, NativeTarget, ObjectFormat};
use target_operations::{
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
            terminal_psi::StructuralPathSegment::FixedByteRange { start, end } => {
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
    digest.raw(&canonical_usize(effect.operation_ordinal));
    digest.raw(&canonical_usize(effect.code_offset));
    digest.raw(&canonical_usize(effect.byte_count));
}

/// Hash the complete retained settlement row so no realization, argument,
/// completion, result, or coordinate field can be substituted underneath a
/// parent identity.
pub(crate) fn hash_boundary_settlement_record(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    settlement: &machine_code::BoundarySettlementRecord,
) -> Result<(), &'static str> {
    match settlement.invocation_site {
        semantic_vocabulary::BoundaryInvocationSite::Operation(operation) => {
            digest.byte(0);
            digest.raw(&operation.get().to_le_bytes());
        }
        semantic_vocabulary::BoundaryInvocationSite::ExternalCompletion(edge) => {
            digest.byte(1);
            digest.raw(&edge.get().to_le_bytes());
        }
    }
    digest.raw(&settlement.boundary.get().to_le_bytes());
    match settlement.execution {
        BoundaryExecutionRecord::AdmittedProvider(record) => {
            digest.byte(1);
            digest.raw(&record.provider_plan_report_identity.to_le_bytes());
            digest.raw(&record.provider_execution_report_identity.to_le_bytes());
            digest.raw(&record.provider_execution_report_fingerprint.to_le_bytes());
            digest.raw(&record.normalized_root_report_identity.to_le_bytes());
            digest.raw(&record.boundary_contract_report_fingerprint.to_le_bytes());
        }
        BoundaryExecutionRecord::CompilerBuiltin(execution) => {
            digest.byte(2);
            digest.byte(compiler_builtin_execution_tag(execution));
            if matches!(
                execution,
                CompilerBuiltinExecution::PlacedScalarAccess { .. }
            ) {
                let mut bytes = Vec::new();
                legalized_operations::legalized_operations::identity::encode_boundary_execution_identity(&mut bytes, target_operations::BoundaryExecutionBinding::CompilerBuiltin(execution));
                digest.raw(&bytes);
            }
        }
    }
    hash_boundary_realization(digest, &settlement.realization);
    digest.raw(&canonical_usize(settlement.scalar_arguments.len()));
    for argument in &settlement.scalar_arguments {
        digest.raw(&argument.source_value.get().to_le_bytes());
        hash_scalar_type(digest, argument.scalar_type);
        hash_integer_value(digest, argument.immediate);
        hash_machine_register(digest, argument.destination);
    }
    digest.raw(&canonical_usize(settlement.runtime_scalar_arguments.len()));
    for argument in &settlement.runtime_scalar_arguments {
        digest.raw(&argument.parameter_index.to_le_bytes());
        hash_internal_scalar_argument_source(digest, &argument.source);
        hash_value_placement(digest, &argument.placement);
        digest.raw(&canonical_usize(argument.code_offset));
        digest.raw(&canonical_usize(argument.byte_count));
    }
    digest.raw(&canonical_usize(settlement.arguments.len()));
    for argument in &settlement.arguments {
        hash_structural_argument(digest, argument);
    }
    digest.raw(&canonical_usize(settlement.byte_sequence_arguments.len()));
    for custody in &settlement.byte_sequence_arguments {
        hash_structural_argument(digest, &custody.argument);
        digest.raw(&custody.literal_operation.get().to_le_bytes());
        let declaration = terminal_codec::encode_structural_type_declaration(
            &custody.structural_type,
        )
        .map_err(|_| "installed D41 byte-sequence declaration cannot be encoded canonically")?;
        hash_bytes(digest, &declaration);
        hash_bytes(digest, &custody.bytes);
        digest.raw(&canonical_usize(custody.code_offset));
        digest.raw(&canonical_usize(custody.code_byte_count));
        digest.raw(&canonical_usize(custody.data_offset));
        digest.raw(&canonical_usize(custody.data_byte_count));
    }
    digest.raw(&canonical_usize(settlement.completion_claim_sources.len()));
    for source in &settlement.completion_claim_sources {
        hash_claim_source(digest, source);
    }
    digest.raw(&canonical_usize(settlement.completion_receipts.len()));
    for receipt in &settlement.completion_receipts {
        digest.raw(&receipt.claim.get().to_le_bytes());
        digest.raw(&receipt.argument_index.to_le_bytes());
        hash_structural_path(digest, &receipt.path);
    }
    digest.raw(&canonical_usize(
        settlement.completion_provider_custody.len(),
    ));
    for binding in &settlement.completion_provider_custody {
        hash_claim_source(digest, &binding.source);
        digest.raw(&binding.receipt.claim.get().to_le_bytes());
        digest.raw(&binding.receipt.argument_index.to_le_bytes());
        let execution = binding.provider_execution;
        digest.raw(&execution.provider_plan_report_identity.to_le_bytes());
        digest.raw(&execution.provider_execution_report_identity.to_le_bytes());
        digest.raw(
            &execution
                .provider_execution_report_fingerprint
                .to_le_bytes(),
        );
        digest.raw(&execution.normalized_root_report_identity.to_le_bytes());
        digest.raw(&execution.boundary_contract_report_fingerprint.to_le_bytes());
    }
    hash_boundary_result_record(digest, &settlement.native_result)?;
    digest.raw(&canonical_usize(settlement.operation_ordinal));
    digest.raw(&canonical_usize(settlement.code_offset));
    digest.raw(&canonical_usize(settlement.byte_count));
    Ok(())
}

const fn compiler_builtin_execution_tag(execution: CompilerBuiltinExecution) -> u8 {
    match execution {
        CompilerBuiltinExecution::PlacedScalarAccess { .. } => 250,
        CompilerBuiltinExecution::HostedExitProcessI32 => 1,
        CompilerBuiltinExecution::HostedReadByte => 2,
        CompilerBuiltinExecution::HostedWriteByteI32 => 3,
        // Tags 4..=27: one per exact operation and format.
        CompilerBuiltinExecution::NearestIeeeFloatArithmetic { operation, format } => {
            4 + 2 * float_arithmetic_tag(operation) + float_format_tag(format)
        }
    }
}

const fn float_arithmetic_tag(operation: target_operations::IeeeFloatArithmeticOperation) -> u8 {
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
        BoundaryRealization::PlacedScalarAccess(access) => {
            digest.byte(9);
            digest.raw(&access.contract);
            let mut bytes = Vec::new();
            legalized_operations::legalized_operations::identity::encode_placed_scalar_demand(
                &mut bytes,
                access.demand,
            );
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
            // Only an x86 FMA carries custody, so appending it stays injective.
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

/// The complete structural-result record body shared by the hosted read-byte
/// parent identity and the installed-settlement record hash.
pub(super) fn hash_boundary_structural_result(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    result: &machine_code::BoundaryStructuralResultRecord,
) -> Result<(), terminal_codec::CodecError> {
    digest.raw(&result.defining_operation.get().to_le_bytes());
    digest.raw(&result.result.place.get().to_le_bytes());
    digest.raw(&result.result.structural_type.get().to_le_bytes());
    digest.byte(match result.result.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    });
    digest.u64(result.result.qualifications.len() as u64);
    for domain in &result.result.qualifications {
        digest.raw(&domain.get().to_le_bytes());
    }
    digest.u64(result.result.projected_qualifications.len() as u64);
    for qualification in &result.result.projected_qualifications {
        hash_structural_path(digest, &qualification.path);
        digest.raw(&qualification.domain.get().to_le_bytes());
    }
    digest.u64(result.result.claims.len() as u64);
    for claim in &result.result.claims {
        digest.raw(&claim.claim.get().to_le_bytes());
        hash_structural_path(digest, &claim.path);
    }
    let declaration = terminal_codec::encode_structural_type_declaration(&result.declaration)?;
    hash_bytes(digest, &declaration);
    hash_sum_layout(digest, &result.layout);
    digest.raw(&result.home_byte_offset.to_le_bytes());
    Ok(())
}

fn hash_scalar_type(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    scalar_type: ScalarType,
) {
    match scalar_type {
        ScalarType::Boolean => digest.byte(1),
        ScalarType::Integer(integer) => {
            digest.byte(2);
            digest.byte(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            digest.raw(&integer.bits().to_le_bytes());
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
    digest.raw(&canonical_usize(placement.locations.len()));
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
    digest.byte(match argument.access {
        terminal_psi::StructuralAccess::Owned => 1,
        terminal_psi::StructuralAccess::SharedBorrow => 2,
        terminal_psi::StructuralAccess::MutableBorrow => 3,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => 4,
    });
    hash_structural_path(digest, &argument.path);
}

/// Hash one declared structural formal: its declared position, place,
/// structural type, multiplicity, access, and exact qualification
/// rows. No declared field can drift underneath a retained argument binding.
pub(crate) fn hash_structural_parameter_declaration(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    parameter: &terminal_psi::StructuralParameterDeclaration,
) {
    digest.raw(&parameter.place.get().to_le_bytes());
    digest.raw(&parameter.position.to_le_bytes());
    digest.raw(&parameter.structural_type.get().to_le_bytes());
    digest.byte(match parameter.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    });
    digest.byte(match parameter.access {
        terminal_psi::StructuralAccess::Owned => 1,
        terminal_psi::StructuralAccess::SharedBorrow => 2,
        terminal_psi::StructuralAccess::MutableBorrow => 3,
        terminal_psi::StructuralAccess::WriteOnlyBorrow => 4,
    });
    digest.raw(&canonical_usize(parameter.qualifications.len()));
    for domain in &parameter.qualifications {
        digest.raw(&domain.get().to_le_bytes());
    }
    digest.raw(&canonical_usize(parameter.projected_qualifications.len()));
    for qualification in &parameter.projected_qualifications {
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
            digest.raw(&canonical_usize(content.input.segments.len()));
            for segment in &content.input.segments {
                match segment {
                    semantic_vocabulary::ContentPlaceSegment::Case(identity) => {
                        digest.byte(1);
                        hash_bytes(digest, identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::Field(identity) => {
                        digest.byte(2);
                        hash_bytes(digest, identity.as_bytes());
                    }
                    semantic_vocabulary::ContentPlaceSegment::FixedIndex(index) => {
                        digest.byte(3);
                        digest.raw(&index.to_le_bytes());
                    }
                }
            }
            digest.raw(&canonical_usize(content.projections.len()));
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
                hash_bytes(digest, projection.algebra.parameter.as_bytes());
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

pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(crate) fn hash_target(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    target: NativeTarget,
) {
    digest.byte(match target.architecture {
        Architecture::Aarch64 => 1,
        Architecture::X86_64 => 2,
    });
    digest.byte(match target.object_format {
        ObjectFormat::Elf => 1,
        ObjectFormat::MachO => 2,
        ObjectFormat::Coff => 3,
    });
    digest.raw(&canonical_usize(target.pointer_size));
    digest.raw(&canonical_usize(target.pointer_alignment));
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
    digest.raw(&canonical_usize(relocation.offset()));
    digest.raw(&canonical_usize(relocation.byte_width()));
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

pub(crate) fn hash_bytes(digest: &mut crate::digest_encoder::DomainDigestEncoder, bytes: &[u8]) {
    digest.raw(&canonical_usize(bytes.len()));
    digest.raw(&bytes);
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
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-physical-evidence-gap.sha256.v1\0",
    );
    match subject {
        NativePhysicalEvidenceGapSubject::ForeignCallSiteOwner { machine, owner } => {
            digest.byte(2);
            digest.raw(&machine.get().to_le_bytes());
            match owner {
                CallSiteOwner::Operation(operation) => {
                    digest.byte(1);
                    digest.raw(&operation.get().to_le_bytes());
                    digest.raw(&0_u32.to_le_bytes());
                }
                CallSiteOwner::ExternalCompletion(edge) => {
                    digest.byte(3);
                    digest.raw(&edge.get().to_le_bytes());
                    digest.raw(&0_u32.to_le_bytes());
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
