//! Reconstruct root backing and the bounded element-scaled offset/length induction.
use super::element_views::{element_backing_pointer, element_stride};
use super::{
    IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use crate::selection::element_view_homes::ElementViewHomes;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::result;
use semantic_vocabulary::IntegerValue;

pub(super) fn create(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ElementViewSubslice {
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
    if row.result.is_some()
        || crate::selection::established_view_input::element_view_type(function, *source)
            != Some(view.structural_type)
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
            .element_views
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
    let stride = element_stride(function, replay, *source).ok_or_else(|| replay.invalid())?;
    // Rejoin the original descriptor load or the already replayed parent view.
    // No proposed operand can assert its own backing identity.
    let backing = element_backing_pointer(replay, row, *source)?;
    let parent = replay
        .transport
        .element_views
        .iter()
        .find(|parent| parent.place == *source)
        .copied();
    // Element bounds s <= e <= L and L * K <= R * K <= root bytes bound the
    // stride product; it is replayed as the machine multiply it selects to.
    let stride_register = result(replay, view.place, 0)?;
    let scaled_start = result(replay, view.place, 0)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(stride)),
        },
        replay.constraints.keys.materialize_i64,
        &[stride_register],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    replay.check_instruction(
        SelectedInstructionKind::WrappingMultiplyI64,
        replay.constraints.keys.multiply_i64,
        &[start_register, stride_register, scaled_start],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    let (byte_offset, root_length) = match parent {
        Some(parent) => {
            let offset = result(replay, view.place, 0)?;
            // The independently reconstructed parent satisfies
            // O + L * K <= R * K. This source's admitted s <= e <= L proves
            // O + s * K <= R * K <= U64::MAX. The current fact is one premise;
            // the exact parent homes and its chain to the root are required
            // for the integer addition theorem.
            replay.check_instruction(
                SelectedInstructionKind::ExactAddI64 {
                    obligation: *obligation,
                    accepted_fact: *accepted_fact,
                },
                replay.constraints.keys.add_i64,
                &[parent.byte_offset, scaled_start, offset],
                &SelectedInstructionProvenance {
                    operations: vec![row.operation],
                    values: vec![*start, *end, *length, parent.root_length],
                    obligations: vec![*obligation],
                    ..Default::default()
                },
            )?;
            (offset, parent.root_length)
        }
        None => (scaled_start, *length),
    };
    let element_length = result(replay, view.place, 8)?;
    // Replaying L' = e - s establishes O' + L' * K = O + e * K <= R * K for the
    // child.
    replay.check_instruction(
        SelectedInstructionKind::ExactSubtractI64 {
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
        replay.constraints.keys.subtract_i64,
        &[end_register, start_register, element_length],
        &SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    replay.transport.element_views.push(ElementViewHomes {
        place: view.place,
        backing_pointer: backing,
        byte_offset,
        element_length,
        root_length,
        element_stride: stride,
    });
    if crate::selection::established_view_input::requires_descriptor(function, view.place) {
        let slot = selected_instructions::LocalStorageSlotId::Structural {
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
        super::local_storage::store(replay, row, slot, 8, element_length)?;
        let descriptor = super::local_storage::address(replay, row, slot, 0, 16, false)?;
        replay.transport.pointers.push((view.place, descriptor));
    }
    Ok(())
}
