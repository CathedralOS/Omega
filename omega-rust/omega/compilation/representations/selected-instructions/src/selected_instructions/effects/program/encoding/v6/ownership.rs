use super::{
    ClaimId, Cursor, EffectLink, ObligationId, OwnershipEvent, PlaceId,
    PreAllocationMachineEffectDecodeError, StructuralTypeId, decode_ids, decode_machine,
};
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
            6 => OwnershipEvent::ExternalCompletionFrontier(decode_claim_ids(cursor)?),
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
            let path = decode_path(cursor)?;
            let structural_type = decode_structural_type(cursor)?;
            let cleanup_machine = decode_machine(cursor)?;
            let consumed_claims = decode_claim_ids(cursor)?;
            let cleanup_receiver = match cursor.byte()? {
                0 => None,
                1 => Some(decode_place(cursor)?),
                _ => return Err(PreAllocationMachineEffectDecodeError::InvalidField),
            };
            let requirement_obligations = decode_ids(cursor, ObligationId::new)?;
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
                terminal_psi::NominalAffineCleanup {
                    place,
                    path,
                    structural_type,
                    cleanup_machine,
                    consumed_claims,
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
            3 => terminal_psi::StructuralPathSegment::Referent,
            4 => terminal_psi::StructuralPathSegment::FixedByteRange {
                start: cursor.u64()?,
                end: cursor.u64()?,
            },
            6 => terminal_psi::StructuralPathSegment::RuntimeIndex {
                index: semantic_vocabulary::ValueId::new(cursor.u64()?)
                    .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
                obligation: ObligationId::new(cursor.u64()?)
                    .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            },
            7 => terminal_psi::StructuralPathSegment::Case(
                semantic_vocabulary::StructuralCaseId::new(cursor.u64()?)
                    .ok_or(PreAllocationMachineEffectDecodeError::InvalidField)?,
            ),
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

pub fn decode_effect_link(
    cursor: &mut Cursor<'_>,
) -> Result<EffectLink, PreAllocationMachineEffectDecodeError> {
    Ok(EffectLink {
        input: cursor.u64()?,
        output: cursor.u64()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{
        StructuralAffineDiscard, StructuralPathSegment, TerminalAffineCleanupAction,
    };

    #[test]
    fn nominal_cleanup_round_trip_retains_projected_path() {
        let mut previous_encoding = None;
        for path in [
            Vec::new(),
            vec![StructuralPathSegment::Field("children".into())],
            vec![
                StructuralPathSegment::Field("children".into()),
                StructuralPathSegment::FixedIndex(1),
            ],
        ] {
            let action =
                TerminalAffineCleanupAction::InvokeNominal(terminal_psi::NominalAffineCleanup {
                    place: PlaceId::new(1).unwrap(),
                    path,
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    cleanup_machine: semantic_vocabulary::MachineId::new(3).unwrap(),
                    consumed_claims: vec![ClaimId::new(4).unwrap()],
                    cleanup_receiver: Some(PlaceId::new(5).unwrap()),
                    requirement_obligations: vec![ObligationId::new(6).unwrap()],
                });
            let mut encoded = Vec::new();
            crate::selected_instructions::effects::program::identity::encode_cleanup(
                &mut encoded,
                &action,
            );
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_cleanup(&mut cursor).unwrap(), action);
            assert_eq!(cursor.remaining(), 0);
            if let Some(previous) = previous_encoding {
                assert_ne!(encoded, previous);
            }
            previous_encoding = Some(encoded);
        }

        let mut retired_envelope = super::super::MAGIC.to_vec();
        retired_envelope.extend_from_slice(&33_u32.to_le_bytes());
        assert_eq!(
            super::super::decode_terminal_pre_allocation_machine_effect_plan(&retired_envelope),
            Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                33
            ))
        );
    }
    #[test]
    fn cleanup_action_round_trip_retains_every_path_segment() {
        let action = TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
            place: PlaceId::new(1).unwrap(),
            structural_type: StructuralTypeId::new(2).unwrap(),
            path: vec![
                StructuralPathSegment::Field("payload".into()),
                StructuralPathSegment::FixedIndex(3),
                StructuralPathSegment::Referent,
                StructuralPathSegment::FixedByteRange { start: 4, end: 8 },
                StructuralPathSegment::RuntimeIndex {
                    index: semantic_vocabulary::ValueId::new(5).unwrap(),
                    obligation: ObligationId::new(6).unwrap(),
                },
                StructuralPathSegment::Case(semantic_vocabulary::StructuralCaseId::new(7).unwrap()),
            ],
        });
        let mut encoded = Vec::new();
        crate::selected_instructions::effects::program::identity::encode_cleanup(
            &mut encoded,
            &action,
        );
        let mut cursor = Cursor::new(&encoded);
        assert_eq!(decode_cleanup(&mut cursor).unwrap(), action);
        assert_eq!(cursor.remaining(), 0);

        let mut retired_path = 1_u64.to_le_bytes().to_vec();
        retired_path.push(5);
        assert_eq!(
            decode_path(&mut Cursor::new(&retired_path)),
            Err(PreAllocationMachineEffectDecodeError::InvalidField)
        );

        let mut retired_envelope = super::super::MAGIC.to_vec();
        retired_envelope.extend_from_slice(&32_u32.to_le_bytes());
        assert_eq!(
            super::super::decode_terminal_pre_allocation_machine_effect_plan(&retired_envelope),
            Err(PreAllocationMachineEffectDecodeError::UnsupportedVersion(
                32
            ))
        );
    }
}
