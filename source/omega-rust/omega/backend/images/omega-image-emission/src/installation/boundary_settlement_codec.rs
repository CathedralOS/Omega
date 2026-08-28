//! Canonical format-36 codec for boundary-settlement rows.
//!
//! The installation parent retains upfront count conversion, settlement order,
//! validation, and admission replay. This child composes the exact row bytes.

use omega_machine_code::{
    BoundaryByteSequenceArgumentRecord, BoundaryResultRecord, BoundarySettlementRecord,
    CompletionProviderCustodyBinding,
};
use omega_target_operations::{
    BoundaryRealization, BoundaryScalarArgument, ClaimCompletionOnlyRealization,
    DirectPortReadU8Realization, LinuxExitGroupI32Realization, LinuxWriteLineRealization,
    MetadataOnlyPortRealization,
};
use psi_core::{
    BoundaryMachineId, ClaimId, EdgeId, IntegerValue, MachineId, OperationId, ServiceId, ValueId,
};
use psi_terminal::CompletionReceipt;

use super::{
    InstallationError, ObjectBoundarySettlement, Reader,
    boundary_result_scalar_codec::{
        decode_boundary_result_scalar_type, encode_boundary_result_scalar_type,
    },
    completion_custody_codec::{decode_completion_claim_source, encode_completion_claim_source},
    decode_boolean, decode_structural_types, encode_structural_types,
    provider_execution_codec::{decode_provider_execution, encode_provider_execution},
    push_u16, push_u32, push_u64,
    structural_argument_codec::{decode_structural_argument, encode_structural_argument},
    value_placement_codec::{decode_placement, decode_register, encode_placement, register_tag},
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
        encode_provider_execution(bytes, settlement.provider_execution);
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
        match &settlement.native_result {
            Some(result) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, result.value.get());
                push_u64(bytes, result.return_edge.get());
                encode_boundary_result_scalar_type(bytes, result.scalar_type);
                encode_placement(bytes, &result.placement)?;
            }
            None => bytes.extend_from_slice(&[0; 4]),
        }
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
        let provider_execution = decode_provider_execution(reader)?;
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
        let native_result_present = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let native_result = if native_result_present {
            Some(BoundaryResultRecord {
                value: ValueId::new(reader.u64()?)
                    .ok_or(InstallationError::InvalidBoundaryResult)?,
                return_edge: EdgeId::new(reader.u64()?)
                    .ok_or(InstallationError::InvalidBoundaryResult)?,
                scalar_type: decode_boundary_result_scalar_type(reader)?,
                placement: decode_placement(reader)?,
            })
        } else {
            None
        };
        boundary_settlements.push(ObjectBoundarySettlement {
            machine,
            settlement: BoundarySettlementRecord {
                psi_operation,
                boundary,
                provider_execution,
                realization,
                scalar_arguments,
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
