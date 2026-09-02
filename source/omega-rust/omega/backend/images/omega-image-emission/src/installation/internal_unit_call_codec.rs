//! Canonical format-62 codec for one installed internal Unit-call row.
//!
//! Call ordering, stack composition, and custody validation remain in the
//! installation parent. This child owns only the exact call-row bytes.

use omega_machine_code::{
    InternalStructuralCallResult, InternalUnitCallArgumentRecord, InternalUnitCallRecord,
    InternalUnitScalarCallArgumentRecord,
};
use omega_target_operations::CallSiteOwner;
use psi_core::{ClaimId, EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId};
use psi_terminal::{
    ClaimTransfer, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralResultClaimBinding, StructuralResultClaimTransfer, StructuralResultDeclaration,
};

use super::{
    InstallationError, InstalledInternalUnitCall, Reader, decode_boolean,
    internal_unit_scalar_call_codec::{decode_argument_source, encode_argument_source},
    push_u16, push_u32, push_u64,
    structural_argument_codec::{decode_structural_argument, encode_structural_argument},
    value_placement_codec::{
        decode_direct_placement, decode_shape, encode_direct_placement, encode_shape,
    },
};

pub(super) fn encode_internal_unit_calls(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[InstalledInternalUnitCall],
) -> Result<(), InstallationError> {
    push_u32(bytes, count);
    for call in installed {
        encode_internal_unit_call(bytes, call)?;
    }
    Ok(())
}

fn encode_internal_unit_call(
    bytes: &mut Vec<u8>,
    installed: &InstalledInternalUnitCall,
) -> Result<(), InstallationError> {
    let custody = &installed.custody;
    push_u64(bytes, installed.machine.get());
    push_u64(
        bytes,
        u64::try_from(installed.text_offset)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    match custody.owner {
        CallSiteOwner::Operation(operation) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, operation.get());
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, edge.get());
            push_u32(bytes, action_ordinal);
            push_u32(bytes, 0);
        }
    }
    push_u64(bytes, custody.target.get());
    match custody.result {
        None => bytes.extend_from_slice(&[0; 6]),
        Some(psi_core::ScalarType::Boolean) => {
            bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
        }
        Some(psi_core::ScalarType::Integer(integer)) => {
            bytes.push(2);
            bytes.push(u8::from(integer.is_address()));
            bytes.push(u8::from(matches!(
                integer.sign(),
                psi_core::IntegerSign::Signed
            )));
            bytes.push(0);
            push_u16(bytes, integer.bits());
        }
        Some(psi_core::ScalarType::IeeeFloat(format)) => {
            bytes.extend_from_slice(&[3, 0, 0, 0]);
            push_u16(
                bytes,
                match format {
                    psi_core::IeeeFloatFormat::Binary32 => 32,
                    psi_core::IeeeFloatFormat::Binary64 => 64,
                },
            );
        }
    }
    match &custody.semantic_result {
        Some(result) if Some(result.scalar_type) == custody.result => {
            bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
            push_u64(bytes, result.value.get());
        }
        None => bytes.extend_from_slice(&[0; 16]),
        Some(_) => {
            return Err(InstallationError::InvalidInternalUnitCall(
                installed.machine,
            ));
        }
    }
    push_u64(
        bytes,
        u64::try_from(custody.operation_ordinal)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(custody.code_offset)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(custody.byte_count)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u32(
        bytes,
        u32::try_from(custody.scalar_arguments.len())
            .map_err(|_| InstallationError::TooManyInternalUnitScalarCallArguments)?,
    );
    for argument in &custody.scalar_arguments {
        push_u32(bytes, argument.parameter_index);
        encode_argument_source(bytes, argument.source)?;
        encode_direct_placement(bytes, &argument.destination)?;
        push_u64(
            bytes,
            u64::try_from(argument.code_offset)
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(argument.byte_count)
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
    }
    push_u32(
        bytes,
        u32::try_from(custody.arguments.len())
            .map_err(|_| InstallationError::TooManyInternalUnitCallArguments)?,
    );
    for argument in &custody.arguments {
        encode_structural_argument(
            bytes,
            &StructuralArgument {
                place: argument.place,
                access: argument.access,
                path: argument.path.clone(),
            },
        )?;
        push_u64(bytes, argument.root_structural_type.get());
        push_u64(bytes, argument.structural_type.get());
        encode_shape(bytes, argument.shape)?;
        push_u32(bytes, argument.source_byte_offset);
        push_u32(bytes, argument.source_home_byte_offset);
        push_u32(bytes, argument.call_stack_bytes);
        match (argument.fixed_array_length, argument.element_stride) {
            (Some(length), Some(stride)) => {
                bytes.push(1);
                bytes.extend_from_slice(&[0; 3]);
                push_u64(bytes, length);
                push_u32(bytes, stride);
            }
            (None, None) => {
                bytes.push(0);
                bytes.extend_from_slice(&[0; 3]);
            }
            _ => {
                return Err(InstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ));
            }
        }
        encode_direct_placement(bytes, &argument.source)?;
        encode_direct_placement(bytes, &argument.destination)?;
        push_u64(
            bytes,
            u64::try_from(argument.code_offset)
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(argument.byte_count)
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u32(
            bytes,
            u32::try_from(argument.bytes.len())
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        bytes.extend_from_slice(&argument.bytes);
    }
    push_u32(
        bytes,
        u32::try_from(custody.claim_transfers.len())
            .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?,
    );
    for transfer in &custody.claim_transfers {
        push_u64(bytes, transfer.claim.get());
        push_u32(bytes, transfer.argument_index);
    }
    encode_structural_result(bytes, installed.machine, custody.structural_result.as_ref())?;
    Ok(())
}

fn encode_structural_result(
    bytes: &mut Vec<u8>,
    machine: MachineId,
    result: Option<&InternalStructuralCallResult>,
) -> Result<(), InstallationError> {
    let Some(result) = result else {
        bytes.extend_from_slice(&[0; 4]);
        return Ok(());
    };
    let claim_bearing_linear = result.operation_result.multiplicity
        == StructuralMultiplicity::Linear
        && result.function_result.multiplicity == StructuralMultiplicity::Linear
        && result.operation_result.structural_type == result.function_result.structural_type
        && result.operation_result.qualifications == result.function_result.qualifications
        && result.operation_result.projected_qualifications.is_empty()
        && result.function_result.projected_qualifications.is_empty()
        && result.operation_result.claims.len() == 1
        && result.operation_result.claims[0].path.is_empty()
        && result.returned_claim_transfers.len() == 1
        && result.returned_claims.len() == 1
        && result.caller_result_placement == result.callee_result_placement;
    let claim_free_affine = result.operation_result.multiplicity == StructuralMultiplicity::Affine
        && result.function_result.multiplicity == StructuralMultiplicity::Affine
        && result.operation_result.structural_type == result.function_result.structural_type
        && result.operation_result.qualifications.is_empty()
        && result.operation_result.projected_qualifications.is_empty()
        && result.operation_result.claims.is_empty()
        && result.function_result.qualifications.is_empty()
        && result.function_result.projected_qualifications.is_empty()
        && result.returned_claim_transfers.is_empty()
        && result.returned_claims.is_empty()
        && result.caller_result_placement == result.callee_result_placement;
    if !claim_bearing_linear && !claim_free_affine {
        return Err(InstallationError::InvalidInternalUnitCall(machine));
    }
    if claim_free_affine {
        bytes.extend_from_slice(&[2, 0, 0, 0]);
        push_u64(bytes, result.operation_result.place.get());
        push_u64(bytes, result.operation_result.structural_type.get());
        push_u64(bytes, result.function_result.place.get());
        push_u64(bytes, result.function_result.structural_type.get());
        encode_direct_placement(bytes, &result.caller_result_placement)?;
        encode_direct_placement(bytes, &result.callee_result_placement)?;
        return Ok(());
    }
    if result.operation_result.multiplicity != StructuralMultiplicity::Linear
        || result.function_result.multiplicity != StructuralMultiplicity::Linear
        || result.operation_result.structural_type != result.function_result.structural_type
        || result.operation_result.qualifications != result.function_result.qualifications
        || result.operation_result.claims.len() != 1
        || !result.operation_result.claims[0].path.is_empty()
        || result.returned_claim_transfers.len() != 1
        || result.returned_claims.len() != 1
        || result.caller_result_placement != result.callee_result_placement
    {
        return Err(InstallationError::InvalidInternalUnitCall(machine));
    }
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    push_u64(bytes, result.operation_result.place.get());
    push_u64(bytes, result.operation_result.structural_type.get());
    push_u32(
        bytes,
        u32::try_from(result.operation_result.qualifications.len())
            .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?,
    );
    for qualification in &result.operation_result.qualifications {
        push_u64(bytes, qualification.get());
    }
    push_u64(bytes, result.operation_result.claims[0].claim.get());
    push_u64(bytes, result.function_result.place.get());
    push_u64(bytes, result.function_result.structural_type.get());
    push_u32(
        bytes,
        u32::try_from(result.function_result.qualifications.len())
            .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?,
    );
    for qualification in &result.function_result.qualifications {
        push_u64(bytes, qualification.get());
    }
    push_u64(bytes, result.returned_claim_transfers[0].callee_claim.get());
    push_u64(bytes, result.returned_claim_transfers[0].caller_claim.get());
    push_u64(bytes, result.returned_claims[0].get());
    encode_direct_placement(bytes, &result.caller_result_placement)?;
    encode_direct_placement(bytes, &result.callee_result_placement)?;
    Ok(())
}

pub(super) fn decode_internal_unit_calls(
    reader: &mut Reader<'_>,
) -> Result<Vec<InstalledInternalUnitCall>, InstallationError> {
    let count =
        usize::try_from(reader.u32()?).map_err(|_| InstallationError::TooManyInternalUnitCalls)?;
    if count > reader.remaining() / 64 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut internal_unit_calls = Vec::with_capacity(count);
    for _ in 0..count {
        internal_unit_calls.push(decode_internal_unit_call(reader)?);
    }
    Ok(internal_unit_calls)
}

fn decode_internal_unit_call(
    reader: &mut Reader<'_>,
) -> Result<InstalledInternalUnitCall, InstallationError> {
    let machine = MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroFunctionIdentity)?;
    let text_offset = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let owner_tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    let owner = match owner_tag {
        1 => CallSiteOwner::Operation(
            OperationId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
        ),
        2 => {
            let edge = EdgeId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let action_ordinal = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            CallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            }
        }
        tag => return Err(InstallationError::InvalidCallSiteOwnerTag(tag)),
    };
    let target =
        MachineId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let result_tag = reader.u8()?;
    let is_address = decode_boolean(reader.u8()?)?;
    let signed = decode_boolean(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(InstallationError::NonzeroReservedField);
    }
    let bits = reader.u16()?;
    let result = match result_tag {
        0 if !is_address && !signed && bits == 0 => None,
        1 if !is_address && !signed && bits == 0 => Some(psi_core::ScalarType::Boolean),
        2 => Some(psi_core::ScalarType::Integer(
            if is_address {
                if signed {
                    return Err(InstallationError::InvalidInternalUnitCall(machine));
                }
                psi_core::IntegerType::address(bits)
            } else {
                psi_core::IntegerType::new(
                    if signed {
                        psi_core::IntegerSign::Signed
                    } else {
                        psi_core::IntegerSign::Unsigned
                    },
                    bits,
                )
            }
            .map_err(|_| InstallationError::InvalidInternalUnitCall(machine))?,
        )),
        3 if !is_address && !signed && bits == 32 => Some(psi_core::ScalarType::IeeeFloat(
            psi_core::IeeeFloatFormat::Binary32,
        )),
        3 if !is_address && !signed && bits == 64 => Some(psi_core::ScalarType::IeeeFloat(
            psi_core::IeeeFloatFormat::Binary64,
        )),
        _ => return Err(InstallationError::InvalidInternalUnitCall(machine)),
    };
    let semantic_result = match decode_boolean(reader.u8()?)? {
        false => {
            if reader.take(7)? != [0; 7] || reader.u64()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            None
        }
        true => {
            if reader.take(7)? != [0; 7] {
                return Err(InstallationError::NonzeroReservedField);
            }
            let value = psi_core::ValueId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            Some(omega_abstract_operations::AbstractResult {
                value,
                scalar_type: result.ok_or(InstallationError::InvalidInternalUnitCall(machine))?,
            })
        }
    };
    let operation_ordinal = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let code_offset = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let byte_count = usize::try_from(reader.u64()?)
        .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let scalar_argument_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitScalarCallArguments)?;
    if scalar_argument_count > reader.remaining() / 48 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut scalar_arguments = Vec::with_capacity(scalar_argument_count);
    for _ in 0..scalar_argument_count {
        scalar_arguments.push(InternalUnitScalarCallArgumentRecord {
            parameter_index: reader.u32()?,
            source: decode_argument_source(reader)?,
            destination: decode_direct_placement(reader)?,
            code_offset: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
            byte_count: usize::try_from(reader.u64()?)
                .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?,
        });
    }
    let argument_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitCallArguments)?;
    if argument_count > reader.remaining() / 80 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut arguments = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        let argument = decode_structural_argument(reader)?;
        let root_structural_type = StructuralTypeId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
        let structural_type = StructuralTypeId::new(reader.u64()?)
            .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
        let shape = decode_shape(reader)?;
        let source_byte_offset = reader.u32()?;
        let source_home_byte_offset = reader.u32()?;
        let call_stack_bytes = reader.u32()?;
        let has_array = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(InstallationError::NonzeroReservedField);
        }
        let (fixed_array_length, element_stride) = if has_array {
            (Some(reader.u64()?), Some(reader.u32()?))
        } else {
            (None, None)
        };
        let source = decode_direct_placement(reader)?;
        let destination = decode_direct_placement(reader)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let encoded_count = usize::try_from(reader.u32()?)
            .map_err(|_| InstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let bytes = reader.take(encoded_count)?.to_vec();
        arguments.push(InternalUnitCallArgumentRecord {
            place: argument.place,
            access: argument.access,
            path: argument.path,
            root_structural_type,
            structural_type,
            shape,
            source_byte_offset,
            source_home_byte_offset,
            call_stack_bytes,
            fixed_array_length,
            element_stride,
            source,
            destination,
            code_offset,
            byte_count,
            bytes,
        });
    }
    let claim_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?;
    if claim_count > reader.remaining() / 12 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut claim_transfers = Vec::with_capacity(claim_count);
    for _ in 0..claim_count {
        claim_transfers.push(ClaimTransfer {
            claim: ClaimId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
            argument_index: reader.u32()?,
        });
    }
    let structural_result = decode_structural_result(reader, machine)?;
    Ok(InstalledInternalUnitCall {
        machine,
        text_offset,
        custody: InternalUnitCallRecord {
            owner,
            target,
            result,
            semantic_result,
            structural_result,
            scalar_arguments,
            arguments,
            claim_transfers,
            operation_ordinal,
            code_offset,
            byte_count,
        },
    })
}

fn decode_structural_result(
    reader: &mut Reader<'_>,
    machine: MachineId,
) -> Result<Option<InternalStructuralCallResult>, InstallationError> {
    let tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(InstallationError::NonzeroReservedField);
    }
    match tag {
        0 => return Ok(None),
        1 => {}
        2 => {
            let operation_place = PlaceId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let structural_type = StructuralTypeId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let function_place = PlaceId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let function_type = StructuralTypeId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
            let caller_result_placement = decode_direct_placement(reader)?;
            let callee_result_placement = decode_direct_placement(reader)?;
            if structural_type != function_type
                || caller_result_placement != callee_result_placement
            {
                return Err(InstallationError::InvalidInternalUnitCall(machine));
            }
            return Ok(Some(InternalStructuralCallResult {
                operation_result: StructuralOperationResult {
                    place: operation_place,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                },
                function_result: StructuralResultDeclaration {
                    place: function_place,
                    structural_type: function_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                returned_claim_transfers: Vec::new(),
                returned_claims: Vec::new(),
                caller_result_placement,
                callee_result_placement,
            }));
        }
        tag => return Err(InstallationError::InvalidPresenceFlag(tag)),
    }
    let operation_place =
        PlaceId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let structural_type = StructuralTypeId::new(reader.u64()?)
        .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let qualification_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?;
    if qualification_count > reader.remaining() / 8 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut qualifications = Vec::with_capacity(qualification_count);
    for _ in 0..qualification_count {
        qualifications.push(
            psi_core::StructuralDomainId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
        );
    }
    let claim =
        ClaimId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let function_place =
        PlaceId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let function_type = StructuralTypeId::new(reader.u64()?)
        .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let function_qualification_count = usize::try_from(reader.u32()?)
        .map_err(|_| InstallationError::TooManyInternalUnitCallClaims)?;
    if function_qualification_count > reader.remaining() / 8 {
        return Err(InstallationError::UnexpectedEnd);
    }
    let mut function_qualifications = Vec::with_capacity(function_qualification_count);
    for _ in 0..function_qualification_count {
        function_qualifications.push(
            psi_core::StructuralDomainId::new(reader.u64()?)
                .ok_or(InstallationError::ZeroInternalUnitCallIdentity)?,
        );
    }
    let callee_claim =
        ClaimId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let caller_claim =
        ClaimId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let returned_claim =
        ClaimId::new(reader.u64()?).ok_or(InstallationError::ZeroInternalUnitCallIdentity)?;
    let caller_result_placement = decode_direct_placement(reader)?;
    let callee_result_placement = decode_direct_placement(reader)?;
    let result = InternalStructuralCallResult {
        operation_result: StructuralOperationResult {
            place: operation_place,
            structural_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications,
            projected_qualifications: Vec::new(),
            claims: vec![StructuralResultClaimBinding {
                claim,
                path: Vec::new(),
            }],
        },
        function_result: StructuralResultDeclaration {
            place: function_place,
            structural_type: function_type,
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: function_qualifications,
            projected_qualifications: Vec::new(),
        },
        returned_claim_transfers: vec![StructuralResultClaimTransfer {
            callee_claim,
            caller_claim,
        }],
        returned_claims: vec![returned_claim],
        caller_result_placement,
        callee_result_placement,
    };
    if result.operation_result.structural_type != result.function_result.structural_type
        || result.operation_result.qualifications != result.function_result.qualifications
        || result.operation_result.claims[0].claim
            != result.returned_claim_transfers[0].caller_claim
        || result.returned_claims[0] != result.returned_claim_transfers[0].caller_claim
        || result.caller_result_placement != result.callee_result_placement
    {
        return Err(InstallationError::InvalidInternalUnitCall(machine));
    }
    Ok(Some(result))
}
