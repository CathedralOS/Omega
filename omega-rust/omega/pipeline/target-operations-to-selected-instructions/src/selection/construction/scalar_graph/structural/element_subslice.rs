//! Retain original backing with exact element-scaled offset and length homes.
use super::element_views::{element_backing_pointer, element_stride};
use super::{
    Builder, IntegerSign, IntegerType, LegalizedScalarFunction, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, ScalarType, SelectedInstructionKind,
    SelectedInstructionProvenance,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::transport_register;
use crate::selection::element_view_homes::ElementViewHomes;
use semantic_vocabulary::IntegerValue;

pub(super) fn create(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::ElementViewSubslice {
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
    if row.result.is_some()
        || crate::selection::established_view_input::element_view_type(function, *source)
            != Some(result.structural_type)
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
            .element_views
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
    let stride = element_stride(function, builder, *source).ok_or_else(invalid)?;
    let backing = element_backing_pointer(builder, row, *source)?;
    let parent = builder
        .transport
        .element_views
        .iter()
        .find(|view| view.place == *source)
        .copied();
    // Scale the element start into the root's byte units. Bounds give
    // s <= e <= L <= R elements and L * stride <= R * stride <= root bytes, so
    // the stride product cannot overflow either.
    let stride_register = transport_register(builder, result.place, 0)?;
    let scaled_start = transport_register(builder, result.place, 0)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(stride)),
        },
        builder.constraints.keys.materialize_i64,
        &[stride_register],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    builder.emit(
        SelectedInstructionKind::WrappingMultiplyI64,
        builder.constraints.keys.multiply_i64,
        &[start_register, stride_register, scaled_start],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            ..Default::default()
        },
    )?;
    let (byte_offset, root_length) = if let Some(parent) = parent {
        let byte_offset = transport_register(builder, result.place, 0)?;
        // With parent byte offset O, element length L, stride K, and root
        // element length R, the retained chain proves O + L * K <= R * K <=
        // U64::MAX. The element bounds s <= e <= L give
        // O + s * K <= O + e * K <= R * K, so this byte-offset sum is exact.
        builder.emit(
            SelectedInstructionKind::ExactAddI64 {
                obligation: *obligation,
                accepted_fact: *accepted_fact,
            },
            builder.constraints.keys.add_i64,
            &[parent.byte_offset, scaled_start, byte_offset],
            SelectedInstructionProvenance {
                operations: vec![row.operation],
                values: vec![*start, *end, *length, parent.root_length],
                obligations: vec![*obligation],
                ..Default::default()
            },
        )?;
        (byte_offset, parent.root_length)
    } else {
        // The first byte offset is the scaled element start itself; no parent
        // offset exists to add, including for an empty suffix at the bound.
        (scaled_start, *length)
    };
    let element_length = transport_register(builder, result.place, 8)?;
    // L' = e - s is exact, and O' + L' * K = O + e * K <= R * K preserves the
    // byte invariant.
    builder.emit(
        SelectedInstructionKind::ExactSubtractI64 {
            obligation: *obligation,
            accepted_fact: *accepted_fact,
        },
        builder.constraints.keys.subtract_i64,
        &[end_register, start_register, element_length],
        SelectedInstructionProvenance {
            operations: vec![row.operation],
            values: vec![*start, *end, *length],
            obligations: vec![*obligation],
            fuel: row.fuel.clone(),
            ..Default::default()
        },
    )?;
    builder.transport.element_views.push(ElementViewHomes {
        place: result.place,
        backing_pointer: backing,
        byte_offset,
        element_length,
        root_length,
        element_stride: stride,
    });
    if crate::selection::established_view_input::requires_descriptor(function, result.place) {
        let slot = selected_instructions::LocalStorageSlotId::Structural {
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
        // B + R * K <= Bound and O + L * K <= R * K imply B + O <= Bound.
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
        super::local_storage::store(builder, row, slot, 8, element_length)?;
        let descriptor = super::local_storage::address(builder, row, slot, 0, 16, false)?;
        builder.transport.pointers.push((result.place, descriptor));
    }
    Ok(())
}
