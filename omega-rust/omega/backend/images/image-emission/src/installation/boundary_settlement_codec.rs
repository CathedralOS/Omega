//! Canonical format-36 codec for boundary-settlement rows.
//!
//! The installation parent retains upfront count conversion, settlement order,
//! validation, and admission replay. This child composes the exact row bytes.

use calling_conventions::{ConventionalSumCaseLayout, ConventionalSumLayout, PackedFieldLayout};
use machine_code::{
    BoundaryByteSequenceArgumentRecord, BoundaryExecutionRecord, BoundaryResultRecord,
    BoundaryScalarResultRecord, BoundarySettlementRecord, BoundaryStructuralResultRecord,
    CompletionProviderCustodyBinding, ForeignCallScalarArgumentRecord,
    InternalUnitScalarArgumentSourceRecord, UnitScalarParameterLocationRecord,
};
use semantic_vocabulary::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerValue, MachineId, OperationId, PlaceId, ServiceId,
    StructuralDomainId, StructuralTypeId, ValueId,
};
use target_operations::{
    BoundaryRealization, BoundaryScalarArgument, ClaimCompletionOnlyRealization,
    CompilerBuiltinExecution, DirectPortReadU8Realization, LinuxExitGroupI32Realization,
    LinuxReadByteRealization, LinuxWriteByteI32Realization, LinuxWriteLineRealization,
    MetadataOnlyPortRealization,
};
use terminal_psi::{
    CompletionReceipt, StructuralOperationResult, StructuralPathQualification,
    StructuralPathSegment, StructuralResultClaimBinding,
};

use super::{
    InstallationError, ObjectBoundarySettlement, Reader,
    boundary_result_scalar_codec::{
        decode_boundary_result_scalar_type, encode_boundary_result_scalar_type,
    },
    completion_custody_codec::{decode_completion_claim_source, encode_completion_claim_source},
    decode_structural_types, encode_structural_types,
    provider_execution_codec::{decode_provider_execution, encode_provider_execution},
    push_u16, push_u32, push_u64,
    structural_argument_codec::{decode_structural_argument, encode_structural_argument},
    structural_scalar_codec::{
        decode_domains, decode_multiplicity, encode_domains, multiplicity_tag,
    },
    unit_scalar_codec::{
        decode_integer_type, decode_integer_value, decode_scalar_type, decode_unit_scalar_home,
        encode_integer_type, encode_integer_value, encode_scalar_type, encode_unit_scalar_home,
    },
    value_placement_codec::{
        decode_placement, decode_register, decode_shape, encode_placement, encode_shape,
        register_tag,
    },
};

pub(super) fn encode_boundary_settlements(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[ObjectBoundarySettlement],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for installed in installed {
        let settlement = &installed.settlement;
        push_u64(bytes, installed.machine.get());
        push_u64(bytes, settlement.psi_operation.get());
        push_u64(bytes, settlement.boundary.get());
        encode_boundary_execution(bytes, settlement.execution);
        match settlement.realization {
            BoundaryRealization::MetadataOnlyPort(realization) => {
                bytes.push(0);
                push_u64(bytes, realization.effect_operation.get());
                push_u64(bytes, realization.service.get());
                push_u16(bytes, realization.port);
                bytes.push(realization.value);
            }
            BoundaryRealization::DirectPortReadU8(realization) => {
                bytes.push(1);
                push_u64(bytes, 0);
                push_u64(bytes, realization.service.get());
                push_u16(bytes, realization.port);
                bytes.push(0);
            }
            BoundaryRealization::LinuxExitGroupI32(_) => {
                bytes.push(2);
                push_u64(bytes, 0);
                push_u64(bytes, 0);
                push_u16(bytes, 0);
                bytes.push(0);
            }
            BoundaryRealization::LinuxWriteLine(_) => {
                bytes.push(3);
                push_u64(bytes, 0);
                push_u64(bytes, 0);
                push_u16(bytes, 0);
                bytes.push(0);
            }
            BoundaryRealization::ClaimCompletionOnly(_) => {
                bytes.push(4);
                push_u64(bytes, 0);
                push_u64(bytes, 0);
                push_u16(bytes, 0);
                bytes.push(0);
            }
            BoundaryRealization::LinuxWriteByteI32(_) => {
                bytes.push(5);
                push_u64(bytes, 0);
                push_u64(bytes, 0);
                push_u16(bytes, 0);
                bytes.push(0);
            }
            BoundaryRealization::LinuxReadByte(_) => {
                bytes.push(6);
                push_u64(bytes, 0);
                push_u64(bytes, 0);
                push_u16(bytes, 0);
                bytes.push(0);
            }
        }
        bytes.push(0);
        push_u64(
            bytes,
            u64::try_from(settlement.operation_ordinal)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(installed.text_offset)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(settlement.code_offset)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(settlement.byte_count)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
        );
        push_u32(
            bytes,
            u32::try_from(settlement.scalar_arguments.len())
                .map_err(|_| InstallationError::TooManySettlementScalarArguments)?,
        );
        for argument in &settlement.scalar_arguments {
            push_u64(bytes, argument.source_value.get());
            encode_boundary_result_scalar_type(bytes, argument.scalar_type);
            match argument.immediate {
                IntegerValue::Signed(value) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&[0; 3]);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                IntegerValue::Unsigned(value) => {
                    bytes.push(2);
                    bytes.extend_from_slice(&[0; 3]);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            bytes.push(register_tag(argument.destination)?);
            bytes.extend_from_slice(&[0; 3]);
        }
        push_u32(
            bytes,
            u32::try_from(settlement.runtime_scalar_arguments.len())
                .map_err(|_| InstallationError::TooManySettlementScalarArguments)?,
        );
        for argument in &settlement.runtime_scalar_arguments {
            push_u32(bytes, argument.parameter_index);
            encode_boundary_runtime_source(bytes, argument.source)?;
            encode_placement(bytes, &argument.placement)?;
            push_u64(
                bytes,
                u64::try_from(argument.code_offset)
                    .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
            );
            push_u64(
                bytes,
                u64::try_from(argument.byte_count)
                    .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
            );
        }
        push_u32(
            bytes,
            u32::try_from(settlement.arguments.len())
                .map_err(|_| InstallationError::TooManySettlementArguments)?,
        );
        for argument in &settlement.arguments {
            encode_structural_argument(bytes, argument)?;
        }
        push_u32(
            bytes,
            u32::try_from(settlement.byte_sequence_arguments.len())
                .map_err(|_| InstallationError::TooManySettlementArguments)?,
        );
        for custody in &settlement.byte_sequence_arguments {
            encode_structural_argument(bytes, &custody.argument)?;
            push_u64(bytes, custody.literal_operation.get());
            encode_structural_types(bytes, std::slice::from_ref(&custody.structural_type))?;
            push_u64(
                bytes,
                u64::try_from(custody.bytes.len())
                    .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
            );
            bytes.extend_from_slice(&custody.bytes);
            for value in [
                custody.code_offset,
                custody.code_byte_count,
                custody.data_offset,
                custody.data_byte_count,
            ] {
                push_u64(
                    bytes,
                    u64::try_from(value)
                        .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
                );
            }
        }
        push_u32(
            bytes,
            u32::try_from(settlement.completion_claim_sources.len())
                .map_err(|_| InstallationError::TooManyCompletionClaimSources)?,
        );
        for source in &settlement.completion_claim_sources {
            encode_completion_claim_source(bytes, source)?;
        }
        push_u32(
            bytes,
            u32::try_from(settlement.completion_receipts.len())
                .map_err(|_| InstallationError::TooManyCompletionReceipts)?,
        );
        for claim in &settlement.completion_receipts {
            push_u64(bytes, claim.claim.get());
            push_u32(bytes, claim.argument_index);
        }
        push_u32(
            bytes,
            u32::try_from(settlement.completion_provider_custody.len())
                .map_err(|_| InstallationError::TooManyCompletionProviderCustody)?,
        );
        for binding in &settlement.completion_provider_custody {
            encode_completion_claim_source(bytes, &binding.source)?;
            push_u64(bytes, binding.receipt.claim.get());
            push_u32(bytes, binding.receipt.argument_index);
            encode_provider_execution(bytes, binding.provider_execution);
        }
        encode_boundary_result(bytes, &settlement.native_result)?;
    }
    Ok(())
}

pub(super) fn decode_boundary_settlements(
    reader: &mut Reader<'_>,
) -> Result<Vec<ObjectBoundarySettlement>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyBoundarySettlements)?;
    let mut boundary_settlements = Vec::with_capacity(count);
    for _ in 0..count {
        let machine = MachineId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroSettlementIdentity("MachineId"))?;
        let psi_operation = OperationId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroSettlementIdentity("OperationId"))?;
        let boundary = BoundaryMachineId::new(reader.u64()?).ok_or(
            InstallationError::ZeroSettlementIdentity("BoundaryMachineId"),
        )?;
        let execution = decode_boundary_execution(reader)?;
        let realization_tag = reader.u8()?;
        let effect_operation = reader.u64()?;
        let service = reader.u64()?;
        let port = reader.u16()?;
        let value = reader.u8()?;
        let realization = match realization_tag {
            0 => BoundaryRealization::MetadataOnlyPort(MetadataOnlyPortRealization {
                effect_operation: OperationId::new(effect_operation).ok_or(
                    InstallationError::ZeroSettlementIdentity("realization OperationId"),
                )?,
                service: ServiceId::new(service).ok_or(
                    InstallationError::ZeroSettlementIdentity("realization ServiceId"),
                )?,
                port,
                value,
            }),
            1 if effect_operation == 0 && value == 0 => {
                BoundaryRealization::DirectPortReadU8(DirectPortReadU8Realization {
                    service: ServiceId::new(service).ok_or(
                        InstallationError::ZeroSettlementIdentity("realization ServiceId"),
                    )?,
                    port,
                })
            }
            2 if effect_operation == 0 && service == 0 && port == 0 && value == 0 => {
                BoundaryRealization::LinuxExitGroupI32(LinuxExitGroupI32Realization)
            }
            3 if effect_operation == 0 && service == 0 && port == 0 && value == 0 => {
                BoundaryRealization::LinuxWriteLine(LinuxWriteLineRealization)
            }
            4 if effect_operation == 0 && service == 0 && port == 0 && value == 0 => {
                BoundaryRealization::ClaimCompletionOnly(ClaimCompletionOnlyRealization)
            }
            5 if effect_operation == 0 && service == 0 && port == 0 && value == 0 => {
                BoundaryRealization::LinuxWriteByteI32(LinuxWriteByteI32Realization)
            }
            6 if effect_operation == 0 && service == 0 && port == 0 && value == 0 => {
                BoundaryRealization::LinuxReadByte(LinuxReadByteRealization)
            }
            _ => return Err(InstallationError::InvalidBoundaryRealizationTag),
        };
        if reader.u8()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        let operation_ordinal = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
        let text_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
        let scalar_argument_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManySettlementScalarArguments)?;
        if scalar_argument_count > reader.remaining() / 36 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut scalar_arguments = Vec::with_capacity(scalar_argument_count);
        for _ in 0..scalar_argument_count {
            let source_value = ValueId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidBoundaryScalarArgument)?;
            let scalar_type = decode_boundary_result_scalar_type(reader)?;
            let immediate_tag = reader.u8()?;
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let raw = <[u8; 16]>::try_from(reader.take(16)?)
                .map_err(|_| InstallationError::UnexpectedEnd)?;
            let immediate = match immediate_tag {
                1 => IntegerValue::Signed(i128::from_le_bytes(raw)),
                2 => IntegerValue::Unsigned(u128::from_le_bytes(raw)),
                _ => return Err(InstallationError::InvalidBoundaryScalarArgument),
            };
            let destination = decode_register(reader.u8()?)?;
            if reader.take(3)? != [0; 3] {
                return Err(InstallationError::NonzeroReservedField);
            }
            scalar_arguments.push(BoundaryScalarArgument {
                source_value,
                scalar_type,
                immediate,
                destination,
            });
        }
        let runtime_scalar_argument_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManySettlementScalarArguments)?;
        if runtime_scalar_argument_count > reader.remaining() / 24 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut runtime_scalar_arguments = Vec::with_capacity(runtime_scalar_argument_count);
        for _ in 0..runtime_scalar_argument_count {
            runtime_scalar_arguments.push(ForeignCallScalarArgumentRecord {
                parameter_index: reader.u32()?,
                source: decode_boundary_runtime_source(reader)?,
                placement: decode_placement(reader)?,
                code_offset: usize::try_from(reader.u64()?)
                    .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
                byte_count: usize::try_from(reader.u64()?)
                    .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?,
            });
        }
        let argument_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManySettlementArguments)?;
        if argument_count > reader.remaining() / 12 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut arguments = Vec::with_capacity(argument_count);
        for _ in 0..argument_count {
            arguments.push(decode_structural_argument(reader)?);
        }
        let byte_sequence_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManySettlementArguments)?;
        if byte_sequence_count > reader.remaining() / 56 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut byte_sequence_arguments = Vec::with_capacity(byte_sequence_count);
        for _ in 0..byte_sequence_count {
            let argument = decode_structural_argument(reader)?;
            let literal_operation = OperationId::new(reader.u64()?).ok_or(
                InstallationError::ZeroSettlementIdentity("literal OperationId"),
            )?;
            let mut structural_types = decode_structural_types(reader)?;
            let [structural_type] = structural_types.as_mut_slice() else {
                return Err(InstallationError::InvalidSettlementArgumentField);
            };
            let structural_type = structural_type.clone();
            let byte_count = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
            let literal_bytes = reader.take(byte_count)?.to_vec();
            let code_offset = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
            let code_byte_count = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
            let data_offset = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
            let data_byte_count = usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::SettlementOffsetNotRepresentable)?;
            byte_sequence_arguments.push(BoundaryByteSequenceArgumentRecord {
                argument,
                literal_operation,
                structural_type,
                bytes: literal_bytes,
                code_offset,
                code_byte_count,
                data_offset,
                data_byte_count,
            });
        }
        let source_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyCompletionClaimSources)?;
        if source_count > reader.remaining() / 24 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut completion_claim_sources = Vec::with_capacity(source_count);
        for _ in 0..source_count {
            completion_claim_sources.push(decode_completion_claim_source(reader)?);
        }
        let claim_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyCompletionReceipts)?;
        if claim_count > reader.remaining() / 12 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut completion_receipts = Vec::with_capacity(claim_count);
        for _ in 0..claim_count {
            completion_receipts.push(CompletionReceipt {
                claim: ClaimId::new(reader.u64()?)
                    .ok_or(InstallationError::ZeroSettlementIdentity("ClaimId"))?,
                argument_index: reader.u32()?,
            });
        }
        let provider_custody_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::TooManyCompletionProviderCustody)?;
        if provider_custody_count > reader.remaining() / 64 {
            return Err(InstallationError::UnexpectedEnd);
        }
        let mut completion_provider_custody = Vec::with_capacity(provider_custody_count);
        for _ in 0..provider_custody_count {
            completion_provider_custody.push(CompletionProviderCustodyBinding {
                source: decode_completion_claim_source(reader)?,
                receipt: CompletionReceipt {
                    claim: ClaimId::new(reader.u64()?)
                        .ok_or(InstallationError::ZeroSettlementIdentity("ClaimId"))?,
                    argument_index: reader.u32()?,
                },
                provider_execution: decode_provider_execution(reader)?,
            });
        }
        let native_result = decode_boundary_result(reader)?;
        boundary_settlements.push(ObjectBoundarySettlement {
            machine,
            settlement: BoundarySettlementRecord {
                psi_operation,
                boundary,
                execution,
                realization,
                scalar_arguments,
                runtime_scalar_arguments,
                arguments,
                byte_sequence_arguments,
                completion_claim_sources,
                completion_receipts,
                completion_provider_custody,
                native_result,
                operation_ordinal,
                code_offset,
                byte_count,
            },
            text_offset,
        });
    }
    Ok(boundary_settlements)
}

fn encode_boundary_result(
    bytes: &mut Vec<u8>,
    result: &BoundaryResultRecord,
) -> Result<(), InstallationError> {
    match result {
        BoundaryResultRecord::Unit => bytes.extend_from_slice(&[0; 4]),
        BoundaryResultRecord::Scalar(result) => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            push_u64(bytes, result.value.get());
            push_u64(bytes, result.return_edge.get());
            encode_boundary_result_scalar_type(bytes, result.scalar_type);
            encode_placement(bytes, &result.placement)?;
        }
        BoundaryResultRecord::Structural(result) => {
            bytes.extend_from_slice(&[2, 0, 0, 0]);
            push_u64(bytes, result.defining_operation.get());
            encode_structural_operation_result(bytes, &result.result)?;
            encode_sum_layout(bytes, &result.layout)?;
            push_u32(bytes, result.home_byte_offset);
        }
    }
    Ok(())
}

fn decode_boundary_result(
    reader: &mut Reader<'_>,
) -> Result<BoundaryResultRecord, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    match tag {
        0 => Ok(BoundaryResultRecord::Unit),
        1 => Ok(BoundaryResultRecord::Scalar(BoundaryScalarResultRecord {
            value: ValueId::new(reader.u64()?).ok_or(InstallationError::InvalidBoundaryResult)?,
            return_edge: EdgeId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidBoundaryResult)?,
            scalar_type: decode_boundary_result_scalar_type(reader)?,
            placement: decode_placement(reader)?,
        })),
        2 => Ok(BoundaryResultRecord::Structural(
            BoundaryStructuralResultRecord {
                defining_operation: OperationId::new(reader.u64()?)
                    .ok_or(InstallationError::InvalidBoundaryResult)?,
                result: decode_structural_operation_result(reader)?,
                layout: decode_sum_layout(reader)?,
                home_byte_offset: reader.u32()?,
            },
        )),
        _ => Err(InstallationError::InvalidBoundaryResult),
    }
}

fn encode_structural_operation_result(
    bytes: &mut Vec<u8>,
    result: &StructuralOperationResult,
) -> Result<(), InstallationError> {
    push_u64(bytes, result.place.get());
    push_u64(bytes, result.structural_type.get());
    bytes.push(multiplicity_tag(result.multiplicity));
    bytes.extend_from_slice(&[0; 3]);
    encode_domains(bytes, &result.qualifications)?;
    encode_projected_qualifications(bytes, &result.projected_qualifications)?;
    push_u32(
        bytes,
        u32::try_from(result.claims.len())
            .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?,
    );
    for claim in &result.claims {
        push_u64(bytes, claim.claim.get());
        encode_structural_path(bytes, &claim.path)?;
    }
    Ok(())
}

fn decode_structural_operation_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralOperationResult, InstallationError> {
    let place = PlaceId::new(reader.u64()?).ok_or(InstallationError::InvalidBoundaryResult)?;
    let structural_type =
        StructuralTypeId::new(reader.u64()?).ok_or(InstallationError::InvalidBoundaryResult)?;
    let multiplicity = decode_multiplicity(reader.u8()?)?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    let qualifications = decode_domains(reader)?;
    let projected_qualifications = decode_projected_qualifications(reader)?;
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?;
    if count > reader.remaining() / 12 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut claims = Vec::with_capacity(count);
    for _ in 0..count {
        claims.push(StructuralResultClaimBinding {
            claim: ClaimId::new(reader.u64()?).ok_or(InstallationError::InvalidBoundaryResult)?,
            path: decode_structural_path(reader)?,
        });
    }
    Ok(StructuralOperationResult {
        place,
        structural_type,
        multiplicity,
        qualifications,
        projected_qualifications,
        claims,
    })
}

fn encode_projected_qualifications(
    bytes: &mut Vec<u8>,
    rows: &[StructuralPathQualification],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(rows.len())
            .map_err(|_| InstallationError::TooManyStructuralQualifications)?,
    );
    for row in rows {
        encode_structural_path(bytes, &row.path)?;
        push_u64(bytes, row.domain.get());
    }
    Ok(())
}

fn decode_projected_qualifications(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathQualification>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyStructuralQualifications)?;
    if count > reader.remaining() / 12 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        rows.push(StructuralPathQualification {
            path: decode_structural_path(reader)?,
            domain: StructuralDomainId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidBoundaryResult)?,
        });
    }
    Ok(rows)
}

fn encode_structural_path(
    bytes: &mut Vec<u8>,
    path: &[StructuralPathSegment],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(path.len())
            .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?,
    );
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                if identity.is_empty() {
                    return Err(InstallationError::InvalidBoundaryResult);
                }
                bytes.extend_from_slice(&[1, 0, 0, 0]);
                push_u32(
                    bytes,
                    u32::try_from(identity.len())
                        .map_err(|_| InstallationError::SettlementArgumentFieldTooLong)?,
                );
                bytes.extend_from_slice(identity.as_bytes());
            }
            StructuralPathSegment::FixedIndex(index) => {
                bytes.extend_from_slice(&[2, 0, 0, 0]);
                push_u64(bytes, *index);
            }
        }
    }
    Ok(())
}

fn decode_structural_path(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathSegment>, InstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManySettlementArgumentPathSegments)?;
    if count > reader.remaining() / 8 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut path = Vec::with_capacity(count);
    for _ in 0..count {
        let tag = reader.u8()?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        path.push(match tag {
            1 => {
                let len = usize::try_from(reader.u32()?)
                    .map_err(|_| InstallationError::SettlementArgumentFieldTooLong)?;
                let identity = std::str::from_utf8(reader.take(len)?)
                    .map_err(|_| InstallationError::InvalidBoundaryResult)?
                    .to_owned();
                if identity.is_empty() {
                    return Err(InstallationError::InvalidBoundaryResult);
                }
                StructuralPathSegment::Field(identity)
            }
            2 => StructuralPathSegment::FixedIndex(reader.u64()?),
            _ => return Err(InstallationError::InvalidBoundaryResult),
        });
    }
    Ok(path)
}

fn encode_sum_layout(
    bytes: &mut Vec<u8>,
    layout: &ConventionalSumLayout,
) -> Result<(), InstallationError> {
    encode_shape(bytes, layout.shape)?;
    push_u16(bytes, layout.tag_byte_offset);
    push_u16(bytes, layout.payload_byte_offset);
    encode_shape(bytes, layout.tag_shape)?;
    encode_packed_fields(bytes, &layout.common_fields)?;
    push_u32(
        bytes,
        u32::try_from(layout.cases.len()).map_err(|_| InstallationError::TooManyStructuralCases)?,
    );
    for case in &layout.cases {
        encode_packed_fields(bytes, &case.fields)?;
    }
    Ok(())
}

fn decode_sum_layout(reader: &mut Reader<'_>) -> Result<ConventionalSumLayout, InstallationError> {
    let shape = decode_shape(reader)?;
    let tag_byte_offset = reader.u16()?;
    let payload_byte_offset = reader.u16()?;
    let tag_shape = decode_shape(reader)?;
    let common_fields = decode_packed_fields(reader)?;
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStructuralCases)?;
    if count > reader.remaining() / 4 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut cases = Vec::with_capacity(count);
    for _ in 0..count {
        cases.push(ConventionalSumCaseLayout {
            fields: decode_packed_fields(reader)?,
        });
    }
    Ok(ConventionalSumLayout {
        shape,
        tag_byte_offset,
        tag_shape,
        common_fields,
        payload_byte_offset,
        cases,
    })
}

fn encode_packed_fields(
    bytes: &mut Vec<u8>,
    fields: &[PackedFieldLayout],
) -> Result<(), InstallationError> {
    push_u32(
        bytes,
        u32::try_from(fields.len()).map_err(|_| InstallationError::TooManyStructuralFields)?,
    );
    for field in fields {
        push_u16(bytes, field.byte_offset);
        push_u16(bytes, 0);
        encode_shape(bytes, field.shape)?;
    }
    Ok(())
}

fn decode_packed_fields(
    reader: &mut Reader<'_>,
) -> Result<Vec<PackedFieldLayout>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyStructuralFields)?;
    if count > reader.remaining() / 12 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        let byte_offset = reader.u16()?;
        if reader.u16()? != 0 {
            return Err(InstallationError::NonzeroReservedField);
        }
        fields.push(PackedFieldLayout {
            byte_offset,
            shape: decode_shape(reader)?,
        });
    }
    Ok(fields)
}

fn encode_boundary_execution(bytes: &mut Vec<u8>, execution: BoundaryExecutionRecord) {
    match execution {
        BoundaryExecutionRecord::AdmittedProvider(execution) => {
            bytes.push(0);
            encode_provider_execution(bytes, execution);
        }
        BoundaryExecutionRecord::CompilerBuiltin(CompilerBuiltinExecution::LinuxExitGroupI32) => {
            bytes.push(1);
        }
        BoundaryExecutionRecord::CompilerBuiltin(CompilerBuiltinExecution::LinuxReadByte) => {
            bytes.push(3);
        }
        BoundaryExecutionRecord::CompilerBuiltin(CompilerBuiltinExecution::LinuxWriteByteI32) => {
            bytes.push(2);
        }
    }
}

fn decode_boundary_execution(
    reader: &mut Reader<'_>,
) -> Result<BoundaryExecutionRecord, InstallationError> {
    match reader.u8()? {
        0 => Ok(BoundaryExecutionRecord::AdmittedProvider(
            decode_provider_execution(reader)?,
        )),
        1 => Ok(BoundaryExecutionRecord::CompilerBuiltin(
            CompilerBuiltinExecution::LinuxExitGroupI32,
        )),
        2 => Ok(BoundaryExecutionRecord::CompilerBuiltin(
            CompilerBuiltinExecution::LinuxWriteByteI32,
        )),
        3 => Ok(BoundaryExecutionRecord::CompilerBuiltin(
            CompilerBuiltinExecution::LinuxReadByte,
        )),
        _ => Err(InstallationError::InvalidBoundaryExecutionTag),
    }
}

fn encode_boundary_runtime_source(
    bytes: &mut Vec<u8>,
    source: InternalUnitScalarArgumentSourceRecord,
) -> Result<(), InstallationError> {
    match source {
        InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            scalar_type,
            location,
        } => {
            bytes.extend_from_slice(&[0, 0, 0, 0]);
            push_u32(bytes, parameter_index);
            push_u64(bytes, source_value.get());
            encode_scalar_type(bytes, scalar_type)?;
            match location {
                UnitScalarParameterLocationRecord::Register(register) => {
                    bytes.push(0);
                    bytes.push(register_tag(register)?);
                    bytes.extend_from_slice(&[0; 2]);
                    push_u32(bytes, 0);
                }
                UnitScalarParameterLocationRecord::IncomingStack { byte_offset } => {
                    bytes.extend_from_slice(&[1, 0, 0, 0]);
                    push_u32(bytes, byte_offset);
                }
            }
        }
        InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            bytes.extend_from_slice(&[1, 0, 0, 0]);
            push_u64(bytes, defining_operation.get());
            push_u64(bytes, source_value.get());
            encode_integer_type(bytes, scalar_type)?;
            encode_integer_value(bytes, value);
        }
        InternalUnitScalarArgumentSourceRecord::BooleanImmediate { .. } => {
            return Err(InstallationError::InvalidBoundaryScalarArgument);
        }
        InternalUnitScalarArgumentSourceRecord::Home(home) => {
            bytes.extend_from_slice(&[2, 0, 0, 0]);
            encode_unit_scalar_home(bytes, home)?;
        }
    }
    Ok(())
}

fn decode_boundary_runtime_source(
    reader: &mut Reader<'_>,
) -> Result<InternalUnitScalarArgumentSourceRecord, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    match tag {
        0 => {
            let parameter_index = reader.u32()?;
            let source_value = ValueId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidBoundaryScalarArgument)?;
            let scalar_type = decode_scalar_type(reader)?;
            let location_tag = reader.u8()?;
            let register_tag_byte = reader.u8()?;
            if reader.take(2)? != [0; 2] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let byte_offset = reader.u32()?;
            let location = match location_tag {
                0 if byte_offset == 0 => {
                    UnitScalarParameterLocationRecord::Register(decode_register(register_tag_byte)?)
                }
                1 if register_tag_byte == 0 => {
                    UnitScalarParameterLocationRecord::IncomingStack { byte_offset }
                }
                _ => return Err(InstallationError::InvalidBoundaryScalarArgument),
            };
            Ok(InternalUnitScalarArgumentSourceRecord::Parameter {
                parameter_index,
                source_value,
                scalar_type,
                location,
            })
        }
        1 => Ok(InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation: OperationId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidBoundaryScalarArgument)?,
            source_value: ValueId::new(reader.u64()?)
                .ok_or(InstallationError::InvalidBoundaryScalarArgument)?,
            scalar_type: decode_integer_type(reader)?,
            value: decode_integer_value(reader)?,
        }),
        2 => Ok(InternalUnitScalarArgumentSourceRecord::Home(
            decode_unit_scalar_home(reader)?,
        )),
        _ => Err(InstallationError::InvalidBoundaryScalarArgument),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_boundary_result_round_trips_with_exact_sum_layout() {
        let layout = calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[vec![], vec![calling_conventions::ValueShape::integer(4, 4)]],
        )
        .expect("ByteRead layout");
        let result = BoundaryResultRecord::Structural(BoundaryStructuralResultRecord {
            defining_operation: OperationId::new(7).unwrap(),
            result: StructuralOperationResult {
                place: PlaceId::new(8).unwrap(),
                structural_type: StructuralTypeId::new(9).unwrap(),
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            layout,
            home_byte_offset: 16,
        });
        let mut bytes = Vec::new();
        encode_boundary_result(&mut bytes, &result).expect("encode structural result");
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_boundary_result(&mut reader).unwrap(), result);
        assert_eq!(reader.remaining(), 0);

        let mut noncanonical = bytes;
        noncanonical[1] = 1;
        assert_eq!(
            decode_boundary_result(&mut Reader::new(&noncanonical)),
            Err(InstallationError::NonzeroReservedField),
        );
    }
}
