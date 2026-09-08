//! Retain original backing with exact integer offset and length homes.
use super::byte_views::backing_pointer;
use super::*;

pub(super) fn create(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ByteSequenceSubslice {
        result,
        source,
        start,
        end,
        length,
        obligation,
        accepted_fact,
    } = &row.kind
    else {
        return Err(invalid());
    };
    let signature = function.structural.as_ref().ok_or_else(invalid)?;
    let [parameter] = signature.parameters.as_slice() else {
        return Err(invalid());
    };
    if row.result.is_some()
        || parameter.semantic.access != StructuralAccess::SharedBorrow
        || result.structural_type != parameter.semantic.structural_type
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == result.place)
        || builder
            .transport
            .views
            .iter()
            .any(|view| view.place == result.place)
    {
        return Err(invalid());
    }
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?);
    let (_, start_register, _, start_type) = builder.resolve(*start).ok_or_else(invalid)?;
    let (_, end_register, _, end_type) = builder.resolve(*end).ok_or_else(invalid)?;
    let (_, _, _, length_type) = builder.resolve(*length).ok_or_else(invalid)?;
    if start_type != integer || end_type != integer || length_type != integer {
        return Err(invalid());
    }
    let backing = backing_pointer(builder, row, *source)?;
    let parent = builder
        .transport
        .views
        .iter()
        .find(|view| view.place == *source)
        .copied();
    let (byte_offset, root_length) = if let Some(parent) = parent {
        let byte_offset = transport_register(builder, result.place, 0)?;
        // With parent offset O, length L, and root length R, the retained chain
        // proves O + L <= R <= U64::MAX. The current bounds s <= e <= L give
        // O + s <= O + e <= R, so this integer addition cannot overflow.
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation: *obligation,
                accepted_fact: *accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[parent.byte_offset, start_register, byte_offset],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end, *length, parent.root_length],
                obligations: vec![*obligation],
                ..Default::default()
            },
        )?;
        (byte_offset, parent.root_length)
    } else {
        // The first offset is already an unsigned scalar home; no address is
        // formed, including for an empty suffix at the address-space bound.
        (start_register, *length)
    };
    let byte_length = transport_register(builder, result.place, 8)?;
    // L' = e - s is exact, and O' + L' = O + e <= R preserves the invariant.
    builder.emit(
        SelectedInstructionKind::ExactSubtractI64 {
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
        builder.constraints.keys.subtract_i64,
        &[end_register, start_register, byte_length],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    builder.transport.views.push(ByteViewHomes {
        place: result.place,
        backing_pointer: backing,
        byte_offset,
        byte_length,
        root_length,
    });
    if crate::selection::established_view_input::called(function, result.place) {
        let slot = selected_instructions::LocalStorageSlotId {
            operation: row.operation,
            place: result.place,
        };
        builder
            .transport
            .local_slots
            .push(selected_instructions::SelectedLocalStorageSlot {
                id: slot,
                byte_size: 16,
                alignment: 8,
            });
        let pointer = transport_register(builder, result.place, 0)?;
        // B + R <= Bound and O + L <= R imply B + O <= Bound.
        // Equality forces L = 0: zero address bits then represent an empty view.
        // This private descriptor calculation is not exact source integer addition.
        builder.emit(
            SelectedInstructionKind::ByteViewAddress,
            builder.constraints.keys.add_i64,
            &[backing, byte_offset, pointer],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end, *length, root_length],
                obligations: vec![*obligation],
                ..Default::default()
            },
        )?;
        super::local_storage::store(builder, row, slot, 0, pointer)?;
        super::local_storage::store(builder, row, slot, 8, byte_length)?;
        let descriptor = super::local_storage::address(builder, row, slot, 0, 16, false)?;
        builder.transport.pointers.push((result.place, descriptor));
    }
    Ok(())
}
