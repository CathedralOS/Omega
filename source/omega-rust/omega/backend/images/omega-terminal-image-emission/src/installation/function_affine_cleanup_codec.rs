//! Canonical format-36 codec for installed function affine cleanup evidence.
//!
//! Function row placement and cleanup canonicality remain in the installation
//! parent. This child owns the exact Unit cleanup and scalar-control list bytes.

use omega_terminal_machine_code::TerminalUnitAffineCleanupRecord;
use psi_core::{EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId};
use psi_terminal::{StructuralAffineDiscard, StructuralArgument, TerminalAffineCleanupAction};

use super::{
    Reader, TerminalInstallationError, decode_structural_types, encode_structural_types, push_u32,
    push_u64,
    structural_argument_codec::{decode_structural_argument, encode_structural_argument},
    trivial_affine_local_codec::{
        decode_trivial_affine_local, decode_trivial_affine_local_type, encode_trivial_affine_local,
        encode_trivial_affine_local_type,
    },
};

pub(super) fn encode_unit_affine_cleanup(
    bytes: &mut Vec<u8>,
    cleanup: &TerminalUnitAffineCleanupRecord,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, cleanup.psi_edge.get());
    encode_structural_types(bytes, &cleanup.structural_types)?;
    push_u32(
        bytes,
        u32::try_from(cleanup.locals.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for (operation, local, local_type) in &cleanup.locals {
        push_u64(bytes, operation.get());
        encode_trivial_affine_local(bytes, local)?;
        encode_trivial_affine_local_type(bytes, local_type)?;
    }
    push_u32(
        bytes,
        u32::try_from(cleanup.actions.len())
            .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?,
    );
    for action in &cleanup.actions {
        match action {
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                bytes.extend_from_slice(&[1, 0, 0, 0]);
                push_u64(bytes, place.get());
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                bytes.extend_from_slice(&[2, 0, 0, 0]);
                encode_structural_argument(
                    bytes,
                    &StructuralArgument {
                        place: discard.place,
                        access: psi_terminal::StructuralAccess::Owned,
                        path: discard.path.clone(),
                    },
                )?;
                push_u64(bytes, discard.structural_type.get());
            }
            TerminalAffineCleanupAction::InvokeNominal(nominal) => {
                bytes.extend_from_slice(&[3, 0, 0, 0]);
                push_u64(bytes, nominal.place.get());
                push_u64(bytes, nominal.structural_type.get());
                push_u64(bytes, nominal.cleanup_machine.get());
            }
        }
    }
    push_u64(
        bytes,
        u64::try_from(cleanup.code_offset)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    push_u64(
        bytes,
        u64::try_from(cleanup.byte_count)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    );
    Ok(())
}

pub(super) fn encode_scalar_control_affine_cleanups(
    bytes: &mut Vec<u8>,
    cleanups: &[TerminalUnitAffineCleanupRecord],
) -> Result<(), TerminalInstallationError> {
    if !cleanups.is_empty() && cleanups.len() < 2 {
        return Err(
            TerminalInstallationError::InvalidScalarControlAffineCleanupCount(cleanups.len()),
        );
    }
    push_u32(
        bytes,
        u32::try_from(cleanups.len()).map_err(|_| {
            TerminalInstallationError::InvalidScalarControlAffineCleanupCount(cleanups.len())
        })?,
    );
    for cleanup in cleanups {
        encode_unit_affine_cleanup(bytes, cleanup)?;
    }
    Ok(())
}

pub(super) fn decode_unit_affine_cleanup(
    reader: &mut Reader<'_>,
) -> Result<TerminalUnitAffineCleanupRecord, TerminalInstallationError> {
    let psi_edge = EdgeId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("Unit cleanup edge"),
    )?;
    let structural_types = decode_structural_types(reader)?;
    let local_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    if local_count > reader.remaining() {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut locals = Vec::with_capacity(local_count);
    for _ in 0..local_count {
        let operation = OperationId::new(reader.u64()?).ok_or(
            TerminalInstallationError::ZeroStructuralReturnIdentity("Unit local operation"),
        )?;
        locals.push((
            operation,
            decode_trivial_affine_local(reader)?,
            decode_trivial_affine_local_type(reader)?,
        ));
    }
    let action_count = usize::try_from(reader.u32()?)
        .map_err(|_| TerminalInstallationError::TooManyStructuralReturnCleanups)?;
    if action_count > reader.remaining() {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut actions = Vec::with_capacity(action_count);
    for _ in 0..action_count {
        let tag = reader.u8()?;
        if reader.u8()? != 0 || reader.u8()? != 0 || reader.u8()? != 0 {
            return Err(TerminalInstallationError::NonzeroReservedField);
        }
        actions.push(match tag {
            1 => TerminalAffineCleanupAction::DiscardRoot(PlaceId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroStructuralReturnIdentity("Unit cleanup place"),
            )?),
            2 => {
                let argument = decode_structural_argument(reader)?;
                let structural_type = StructuralTypeId::new(reader.u64()?).ok_or(
                    TerminalInstallationError::ZeroStructuralReturnIdentity(
                        "residual Unit cleanup type",
                    ),
                )?;
                TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                    place: argument.place,
                    path: argument.path,
                    structural_type,
                })
            }
            3 => TerminalAffineCleanupAction::InvokeNominal(psi_terminal::NominalAffineCleanup {
                place: PlaceId::new(reader.u64()?).ok_or(
                    TerminalInstallationError::ZeroStructuralReturnIdentity(
                        "nominal Unit cleanup place",
                    ),
                )?,
                structural_type: StructuralTypeId::new(reader.u64()?).ok_or(
                    TerminalInstallationError::ZeroStructuralReturnIdentity(
                        "nominal Unit cleanup type",
                    ),
                )?,
                cleanup_machine: MachineId::new(reader.u64()?).ok_or(
                    TerminalInstallationError::ZeroStructuralReturnIdentity(
                        "nominal Unit cleanup machine",
                    ),
                )?,
                cleanup_receiver: None,
                requirement_obligations: Vec::new(),
            }),
            tag => return Err(TerminalInstallationError::InvalidCleanupActionTag(tag)),
        });
    }
    Ok(TerminalUnitAffineCleanupRecord {
        structural_types,
        psi_edge,
        locals,
        actions,
        code_offset: usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
        byte_count: usize::try_from(reader.u64()?)
            .map_err(|_| TerminalInstallationError::StructuralReturnOffsetNotRepresentable)?,
    })
}

pub(super) fn decode_scalar_control_affine_cleanups(
    reader: &mut Reader<'_>,
) -> Result<Vec<TerminalUnitAffineCleanupRecord>, TerminalInstallationError> {
    let count = usize::try_from(reader.u32()?).map_err(|_| {
        TerminalInstallationError::InvalidScalarControlAffineCleanupCount(usize::MAX)
    })?;
    if count == 1 {
        return Err(TerminalInstallationError::InvalidScalarControlAffineCleanupCount(count));
    }
    // Even an empty cleanup record needs its edge, three collection counts,
    // code offset, and byte count. Reject impossible capacities before
    // allocating from an untrusted installation image.
    const MINIMUM_ENCODED_CLEANUP_BYTES: usize = 36;
    if count > reader.remaining() / MINIMUM_ENCODED_CLEANUP_BYTES {
        return Err(TerminalInstallationError::UnexpectedEnd);
    }
    let mut cleanups = Vec::with_capacity(count);
    for _ in 0..count {
        cleanups.push(decode_unit_affine_cleanup(reader)?);
    }
    Ok(cleanups)
}
