use super::*;

pub fn decode_ownership(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<OwnershipEvent>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut ownership = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let event = match cursor.byte()? {
            1 => OwnershipEvent::ClaimTransfer(decode_claim_ids(cursor)?),
            2 => OwnershipEvent::ClaimCompletion(decode_claim_ids(cursor)?),
            3 => {
                let action_count = cursor.length()?;
                let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
                for _ in 0..action_count {
                    actions.push(decode_cleanup(cursor)?);
                }
                OwnershipEvent::Cleanup(actions)
            }
            4 => OwnershipEvent::StructuralReturn(decode_claim_ids(cursor)?),
            5 => OwnershipEvent::CrashFrontier(decode_claim_ids(cursor)?),
            _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
        };
        ownership.push(event);
    }
    Ok(ownership)
}

fn decode_claim_ids(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<ClaimId>, PreAllocationMachineEffectDecodeError> {
    decode_ids(cursor, ClaimId::new)
}

fn decode_cleanup(
    cursor: &mut Cursor<'_>,
) -> Result<terminal_psi::TerminalAffineCleanupAction, PreAllocationMachineEffectDecodeError> {
    Ok(match cursor.byte()? {
        1 => terminal_psi::TerminalAffineCleanupAction::DiscardRoot(decode_place(cursor)?),
        2 => terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
            terminal_psi::StructuralAffineDiscard {
                place: decode_place(cursor)?,
                path: decode_path(cursor)?,
                structural_type: decode_structural_type(cursor)?,
            },
        ),
        3 => {
            let place = decode_place(cursor)?;
            let structural_type = decode_structural_type(cursor)?;
            let cleanup_machine = decode_machine(cursor)?;
            let cleanup_receiver = match cursor.byte()? {
                0 => None,
                1 => Some(decode_place(cursor)?),
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let requirement_obligations = decode_ids(cursor, ObligationId::new)?;
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
                terminal_psi::NominalAffineCleanup {
                    place,
                    structural_type,
                    cleanup_machine,
                    cleanup_receiver,
                    requirement_obligations,
                },
            )
        }
        _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
    })
}

fn decode_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<terminal_psi::StructuralPathSegment>, PreAllocationMachineEffectDecodeError> {
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        path.push(match cursor.byte()? {
            1 => {
                let length = cursor.length()?;
                let bytes = cursor.take(length)?;
                let name = std::str::from_utf8(bytes)
                    .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)?;
                terminal_psi::StructuralPathSegment::Field(name.to_owned())
            }
            2 => terminal_psi::StructuralPathSegment::FixedIndex(cursor.u64()?),
            _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
        });
    }
    Ok(path)
}

fn decode_place(cursor: &mut Cursor<'_>) -> Result<PlaceId, PreAllocationMachineEffectDecodeError> {
    PlaceId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}

fn decode_structural_type(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralTypeId, PreAllocationMachineEffectDecodeError> {
    StructuralTypeId::new(cursor.u64()?).ok_or(PreAllocationMachineEffectDecodeError::InvalidField)
}
