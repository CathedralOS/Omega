//! Reconstruct root backing and the bounded integer offset/length induction.
use super::byte_views::backing_pointer;
use super::*;

pub(super) fn create(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceSubslice {
        result: view,
        source,
        start,
        end,
        length,
        obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    let signature = function
        .structural
        .as_ref()
        .ok_or_else(|| replay.invalid())?;
    let [parameter] = signature.parameters.as_slice() else {
        return Err(replay.invalid());
    };
    if row.result.is_some()
        || parameter.semantic.access != StructuralAccess::SharedBorrow
        || view.structural_type != parameter.semantic.structural_type
        || view.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !view.qualifications.is_empty()
        || !view.projected_qualifications.is_empty()
        || !view.claims.is_empty()
        || replay
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == view.place)
        || replay
            .transport
            .views
            .iter()
            .any(|parent| parent.place == view.place)
    {
        return Err(replay.invalid());
    }
    let integer = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| replay.invalid())?,
    );
    let (_, start_register, _, start_type) =
        replay.resolve(*start).ok_or_else(|| replay.invalid())?;
    let (_, end_register, _, end_type) = replay.resolve(*end).ok_or_else(|| replay.invalid())?;
    let (_, _, _, length_type) = replay.resolve(*length).ok_or_else(|| replay.invalid())?;
    if [start_type, end_type, length_type]
        .iter()
        .any(|scalar_type| *scalar_type != integer)
    {
        return Err(replay.invalid());
    }
    // Rejoin the original descriptor load or the already replayed parent view.
    // No proposed operand can assert its own backing identity.
    let backing = backing_pointer(replay, row, *source)?;
    let parent = replay
        .transport
        .views
        .iter()
        .find(|parent| parent.place == *source)
        .copied();
    let (byte_offset, root_length) = match parent {
        Some(parent) => {
            let offset = result(replay, view.place, 0)?;
            // The independently reconstructed parent satisfies O + L <= R.
            // This source's admitted s <= e <= L proves O + s <= R <= U64::MAX.
            // The current fact is one premise; the exact parent homes and its
            // chain to the root are required for the integer addition theorem.
            replay.check_instruction(
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                replay.constraints.keys.add_i64,
                &[parent.byte_offset, start_register, offset],
                &SelectedInstructionProvenance {
                    operations: vec![row.operation],
                    values: vec![*start, *end, *length, parent.root_length],
                    obligations: vec![*obligation],
                    ..Default::default()
                },
            )?;
            (offset, parent.root_length)
        }
        None => (start_register, *length),
    };
    let byte_length = result(replay, view.place, 8)?;
    // Replaying L' = e - s establishes O' + L' = O + e <= R for the child.
    replay.check_instruction(
        SelectedInstructionKind::ExactSubtractI64 {
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
        replay.constraints.keys.subtract_i64,
        &[end_register, start_register, byte_length],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    replay.transport.views.push(ByteViewHomes {
        place: view.place,
        backing_pointer: backing,
        byte_offset,
        byte_length,
        root_length,
    });
    if crate::selection::established_view_input::called(function, view.place) {
        let slot = selected_instructions::LocalStorageSlotId {
            operation: row.operation,
            place: view.place,
        };
        replay
            .transport
            .local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        let pointer = result(replay, view.place, 0)?;
        // The retained root/offset induction permits only an empty one-past
        // wrap. No source integer exactness or additional access is asserted.
        replay.check_instruction(
            SelectedInstructionKind::ByteViewAddress,
            replay.constraints.keys.add_i64,
            &[backing, byte_offset, pointer],
            &SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end, *length, root_length],
                obligations: vec![*obligation],
                ..Default::default()
            },
        )?;
        super::local_storage::store(replay, row, slot, 0, pointer)?;
        super::local_storage::store(replay, row, slot, 8, byte_length)?;
        let descriptor = super::local_storage::address(replay, row, slot, 0, 16, false)?;
        replay.transport.pointers.push((view.place, descriptor));
    }
    Ok(())
}
