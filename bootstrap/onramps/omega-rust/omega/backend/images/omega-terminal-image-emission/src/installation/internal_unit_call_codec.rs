//! Canonical format-36 codec for one installed internal Unit-call row.
//!
//! Call ordering, stack composition, and custody validation remain in the
//! installation parent. This child owns only the exact call-row bytes.

use omega_terminal_machine_code::{
    TerminalInternalUnitCallArgumentRecord, TerminalInternalUnitCallRecord,
};
use omega_terminal_target_operations::TerminalCallSiteOwner;
use psi_core::{ClaimId, EdgeId, MachineId, OperationId, StructuralTypeId};
use psi_terminal::{ClaimTransfer, StructuralArgument};

use super::{
    Reader, TerminalInstallationError, TerminalInstalledInternalUnitCall, decode_boolean, push_u16,
    push_u32, push_u64,
    structural_argument_codec::{decode_structural_argument, encode_structural_argument},
    value_placement_codec::{
        decode_direct_placement, decode_shape, encode_direct_placement, encode_shape,
    },
};

pub(super) fn encode_internal_unit_calls(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[TerminalInstalledInternalUnitCall],
) -> Result<(), TerminalInstallationError> {
    push_u32(bytes, count);
    for call in installed {
        encode_internal_unit_call(bytes, call)?;
    }
    Ok(())
}

fn encode_internal_unit_call(
    bytes: &mut Vec<u8>,
    installed: &TerminalInstalledInternalUnitCall,
) -> Result<(), TerminalInstallationError> {
    let custody = &installed.custody;
    push_u64(bytes, installed.machine.get());
    push_u64(
        bytes,
        u64::try_from(installed.text_offset)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    match custody.owner {
        TerminalCallSiteOwner::Operation(operation) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 3]);
            push_u64(bytes, operation.get());
        }
        TerminalCallSiteOwner::CleanupAction {
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
    }
    push_u64(
        bytes,
        u64::try_from(custody.operation_ordinal)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(custody.code_offset)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(custody.byte_count)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
    );
    push_u32(
        bytes,
        u32::try_from(custody.arguments.len())
            .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallArguments)?,
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
                return Err(TerminalInstallationError::InvalidInternalUnitCall(
                    installed.machine,
                ));
            }
        }
        encode_direct_placement(bytes, &argument.source)?;
        encode_direct_placement(bytes, &argument.destination)?;
        push_u64(
            bytes,
            u64::try_from(argument.code_offset)
                .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u64(
            bytes,
            u64::try_from(argument.byte_count)
                .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        push_u32(
            bytes,
            u32::try_from(argument.bytes.len())
                .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?,
        );
        bytes.extend_from_slice(&argument.bytes);
    }
    push_u32(
        bytes,
        u32::try_from(custody.claim_transfers.len())
            .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallClaims)?,
    );
    for transfer in &custody.claim_transfers {
        push_u64(bytes, transfer.claim.get());
        push_u32(bytes, transfer.argument_index);
    }
    Ok(())
}

pub(super) fn decode_internal_unit_calls(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalInstalledInternalUnitCall>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCalls)?;
    if count > reader.remaining() / 64 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut internal_unit_calls = Vec::with_capacity(count);
    for _ in 0..count {
        internal_unit_calls.push(decode_internal_unit_call(reader)?);
    }
    Ok(internal_unit_calls)
}

fn decode_internal_unit_call(
    reader: &mut Reader<'_>,
) -> Result<TerminalInstalledInternalUnitCall, TerminalInstallationError> {
    let machine =
        MachineId::new(reader.u64()?).ok_or(TerminalInstallationError::ZeroFunctionIdentity)?;
    let text_offset = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let owner_tag = reader.u8()?;
    if reader.take(3)? != [0; 3] {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let owner = match owner_tag {
        1 => TerminalCallSiteOwner::Operation(
            OperationId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?,
        ),
        2 => {
            let edge = EdgeId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
            let action_ordinal = reader.u32()?;
            if reader.u32()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            TerminalCallSiteOwner::CleanupAction {
                edge,
                action_ordinal,
            }
        }
        tag => return Err(TerminalInstallationError::InvalidCallSiteOwnerTag(tag)),
    };
    let target = MachineId::new(reader.u64()?)
        .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
    let result_tag = reader.u8()?;
    let is_address = decode_boolean(reader.u8()?)?;
    let signed = decode_boolean(reader.u8()?)?;
    if reader.u8()? != 0 {
        return Err(TerminalInstallationError::NonzeroReservedField);
    }
    let bits = reader.u16()?;
    let result = match result_tag {
        0 if !is_address && !signed && bits == 0 => None,
        1 if !is_address && !signed && bits == 0 => Some(psi_core::ScalarType::Boolean),
        2 => Some(psi_core::ScalarType::Integer(
            if is_address {
                if signed {
                    return Err(TerminalInstallationError::InvalidInternalUnitCall(machine));
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
            .map_err(|_| TerminalInstallationError::InvalidInternalUnitCall(machine))?,
        )),
        _ => return Err(TerminalInstallationError::InvalidInternalUnitCall(machine)),
    };
    let operation_ordinal = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let code_offset = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let byte_count = usize::try_from(reader.u64()?)
        .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
    let argument_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallArguments)?;
    if argument_count > reader.remaining() / 80 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut arguments = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        let argument = decode_structural_argument(reader)?;
        let root_structural_type = StructuralTypeId::new(reader.u64()?)
            .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
        let structural_type = StructuralTypeId::new(reader.u64()?)
            .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?;
        let shape = decode_shape(reader)?;
        let source_byte_offset = reader.u32()?;
        let source_home_byte_offset = reader.u32()?;
        let call_stack_bytes = reader.u32()?;
        let has_array = decode_boolean(reader.u8()?)?;
        if reader.take(3)? != [0; 3] {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        let (fixed_array_length, element_stride) = if has_array {
            (Some(reader.u64()?), Some(reader.u32()?))
        } else {
            (None, None)
        };
        let source = decode_direct_placement(reader)?;
        let destination = decode_direct_placement(reader)?;
        let code_offset = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let byte_count = usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let encoded_count = usize::try_from(reader.u32()?)
            .map_err(|_| TerminalInstallationError::InternalUnitCallOffsetNotRepresentable)?;
        let bytes = reader.take(encoded_count)?.to_vec();
        arguments.push(TerminalInternalUnitCallArgumentRecord {
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
        .map_err(|_| TerminalInstallationError::TooManyInternalUnitCallClaims)?;
    if claim_count > reader.remaining() / 12 {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut claim_transfers = Vec::with_capacity(claim_count);
    for _ in 0..claim_count {
        claim_transfers.push(ClaimTransfer {
            claim: ClaimId::new(reader.u64()?)
                .ok_or(TerminalInstallationError::ZeroInternalUnitCallIdentity)?,
            argument_index: reader.u32()?,
        });
    }
    Ok(TerminalInstalledInternalUnitCall {
        machine,
        text_offset,
        custody: TerminalInternalUnitCallRecord {
            owner,
            target,
            result,
            arguments,
            claim_transfers,
            operation_ordinal,
            code_offset,
            byte_count,
        },
    })
}
