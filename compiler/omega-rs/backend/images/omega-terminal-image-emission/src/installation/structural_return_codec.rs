//! Canonical format-32 codec for one installed structural-return row.
//!
//! Row ordering, function association, and structural-return validation remain
//! in the installation parent. This child owns only the exact row bytes.

use omega_terminal_machine_code::TerminalStructuralReturnRecord;
use psi_core::{ClaimId, EdgeId, MachineId, OperationId, PlaceId};

use super::{
    Reader, TerminalInstallationError, TerminalInstalledStructuralReturn, push_u32, push_u64,
    structural_signature_codec::{
        decode_structural_parameter, decode_structural_result, encode_structural_parameter,
        encode_structural_result,
    },
    trivial_affine_local_codec::{
        decode_trivial_affine_local, decode_trivial_affine_local_type, encode_trivial_affine_local,
        encode_trivial_affine_local_type,
    },
    value_placement_codec::{decode_placement, decode_shape, encode_placement, encode_shape},
};

pub(super) fn encode_structural_returns(
    bytes: &mut Vec<u8>,
    count: u32,
    installed: &[TerminalInstalledStructuralReturn],
) -> Result<(), TerminalInstallationError> {
    push_u32(bytes, count);
    for returned in installed {
        encode_structural_return(bytes, returned)?;
    }
    Ok(())
}

fn encode_structural_return(
    bytes: &mut Vec<u8>,
    installed: &TerminalInstalledStructuralReturn,
) -> Result<(), TerminalInstallationError> {
    let returned = &installed.returned;
    push_u64(bytes, installed.machine.get());
    push_u64(bytes, returned.psi_edge.get());
    push_u32(
        bytes,
        u32::try_from(returned.parameters.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
    );
    for parameter in &returned.parameters {
        encode_structural_parameter(bytes, parameter)?;
    }
    push_u32(
        bytes,
        u32::try_from(returned.parameter_placements.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?,
    );
    for placement in &returned.parameter_placements {
        encode_placement(bytes, placement)?;
    }
    encode_structural_parameter(bytes, &returned.source)?;
    encode_structural_result(bytes, &returned.result)?;
    encode_shape(bytes, returned.shape)?;
    encode_placement(bytes, &returned.source_placement)?;
    encode_placement(bytes, &returned.result_placement)?;
    push_u32(
        bytes,
        u32::try_from(returned.returned_claims.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnClaims)?,
    );
    for claim in &returned.returned_claims {
        push_u64(bytes, claim.get());
    }
    push_u32(
        bytes,
        u32::try_from(returned.trivial_affine_locals.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for (operation, local, local_type) in &returned.trivial_affine_locals {
        push_u64(bytes, operation.get());
        encode_trivial_affine_local(bytes, local)?;
        encode_trivial_affine_local_type(bytes, local_type)?;
    }
    push_u32(
        bytes,
        u32::try_from(returned.trivial_affine_discards.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for place in &returned.trivial_affine_discards {
        push_u64(bytes, place.get());
    }
    push_u64(
        bytes,
        u64::try_from(returned.code_offset)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(returned.byte_count)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    Ok(())
}

pub(super) fn decode_structural_returns(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalInstalledStructuralReturn>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturns)?;
    let mut structural_returns = Vec::with_capacity(count);
    for _ in 0..count {
        structural_returns.push(decode_structural_return(reader)?);
    }
    Ok(structural_returns)
}

fn decode_structural_return(
    reader: &mut Reader<'_>,
) -> Result<TerminalInstalledStructuralReturn, TerminalInstallationError> {
    let machine =
        MachineId::new(reader.u64()?).ok_or(TerminalInstallationError::ZeroFunctionIdentity)?;
    let psi_edge = EdgeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("edge"),
    )?;
    let parameter_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        parameters.push(decode_structural_parameter(reader)?);
    }
    let placement_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnParameters)?;
    let mut parameter_placements = Vec::with_capacity(placement_count);
    for _ in 0..placement_count {
        parameter_placements.push(decode_placement(reader)?);
    }
    let source = decode_structural_parameter(reader)?;
    let result = decode_structural_result(reader)?;
    let shape = decode_shape(reader)?;
    let source_placement = decode_placement(reader)?;
    let result_placement = decode_placement(reader)?;
    let claim_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnClaims)?;
    let mut returned_claims = Vec::with_capacity(claim_count);
    for _ in 0..claim_count {
        returned_claims.push(ClaimId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("claim"),
        )?);
    }
    let local_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    let mut trivial_affine_locals = Vec::with_capacity(local_count);
    for _ in 0..local_count {
        let operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("local operation"),
        )?;
        trivial_affine_locals.push((
            operation,
            decode_trivial_affine_local(reader)?,
            decode_trivial_affine_local_type(reader)?,
        ));
    }
    let cleanup_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    let mut trivial_affine_discards = Vec::with_capacity(cleanup_count);
    for _ in 0..cleanup_count {
        trivial_affine_discards.push(PlaceId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("cleanup place"),
        )?);
    }
    Ok(TerminalInstalledStructuralReturn {
        machine,
        returned: TerminalStructuralReturnRecord {
            psi_edge,
            parameters,
            parameter_placements,
            source,
            result,
            shape,
            source_placement,
            result_placement,
            returned_claims,
            trivial_affine_locals,
            trivial_affine_discards,
            code_offset: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
            byte_count: usize::try_from(reader.u64()?)
                .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
        },
    })
}
