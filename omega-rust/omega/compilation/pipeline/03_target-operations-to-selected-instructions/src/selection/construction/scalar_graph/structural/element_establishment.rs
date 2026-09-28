//! Establish a shared element-view descriptor over a fixed-array field.
use super::local_storage::{address, store};
use super::{
    Builder, LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
    SelectedInstructionKind, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::construction::scalar_graph::structural::invalid;
use crate::selection::construction::scalar_graph::structural::provenance;
use crate::selection::construction::scalar_graph::structural::transport_register;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

pub(super) fn establish(
    function: &LegalizedScalarFunction,
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishElementView {
        result,
        destination,
        source,
        element,
    } = &row.kind
    else {
        return Err(invalid());
    };
    if row.result.is_some()
        || result.place != *destination
        || result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || source.access != StructuralAccess::SharedBorrow
        || source.path.is_empty()
        || builder
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == *destination)
    {
        return Err(invalid());
    }
    let Some(signature) = function.structural.as_ref() else {
        return Err(invalid());
    };
    let Some(place) = signature
        .structural_places
        .iter()
        .find(|place| place.id == *destination)
    else {
        return Err(invalid());
    };
    if !matches!(
        place.kind,
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } if producer == row.operation && structural_type == result.structural_type
    ) {
        return Err(invalid());
    }
    let Some(declaration) = signature
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
    else {
        return Err(invalid());
    };
    let StructuralTypeShape::ElementView {
        element: view_element,
    } = &declaration.shape
    else {
        return Err(invalid());
    };
    if *view_element != *element {
        return Err(invalid());
    }
    let Some(parameter) = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == source.place)
    else {
        return Err(invalid());
    };
    let Some((field_offset, element_count, _stride)) =
        crate::structural_inputs::structural_reference_input::fixed_element_array_view(
            &parameter.semantic,
            source,
            result.structural_type,
            &signature.structural_types,
        )
    else {
        return Err(invalid());
    };
    let Some(base) = builder
        .transport
        .pointers
        .iter()
        .find(|(place, _)| *place == source.place)
        .map(|(_, register)| *register)
    else {
        return Err(invalid());
    };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: *destination,
    };
    builder
        .transport
        .local_slots
        .push(SelectedLocalStorageSlot {
            id: slot,
            byte_size: 16,
            alignment: 8,
        });
    let backing = if field_offset == 0 {
        base
    } else {
        let offset = constant(builder, row, 0, u64::from(field_offset))?;
        let backing = transport_register(builder, *destination, 0)?;
        builder.emit(
            SelectedInstructionKind::ByteViewAddress,
            builder.constraints.keys.add_i64,
            &[base, offset, backing],
            provenance(row),
        )?;
        backing
    };
    store(builder, row, slot, 0, backing)?;
    let length = constant(builder, row, 8, element_count)?;
    store(builder, row, slot, 8, length)?;
    let descriptor = address(builder, row, slot, 0, 16, true)?;
    builder.transport.pointers.push((*destination, descriptor));
    Ok(())
}

fn constant(
    builder: &mut Builder<'_>,
    row: &LegalizedScalarInstruction,
    offset: u32,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishElementView { destination, .. } = &row.kind else {
        return Err(invalid());
    };
    let register = transport_register(builder, *destination, offset)?;
    builder.emit(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(value)),
        },
        builder.constraints.keys.materialize_i64,
        &[register],
        provenance(row),
    )?;
    Ok(register)
}
