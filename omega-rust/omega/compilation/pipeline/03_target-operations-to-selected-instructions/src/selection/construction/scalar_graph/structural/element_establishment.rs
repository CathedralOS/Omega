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
        // A mutable loan's result carries the exclusive custody's affine
        // multiplicity; the descriptor's 16-byte shape is custody-independent.
        || !matches!(
            result.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        )
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !matches!(
            source.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
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
    // The backing is an incoming parameter's storage or a local aggregate
    // home. A home presents its own owned storage through this view's loan
    // and lends the address of its activation slot.
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == source.place)
        .map(|parameter| &parameter.semantic);
    let home = crate::selection::aggregate_result_input::home(function, source.place)
        .map(|(_, home)| local_home_root(home));
    let Some(root) = parameter.or(home.as_ref()) else {
        return Err(invalid());
    };
    let Some((field_offset, element_count, _stride)) =
        crate::structural_inputs::structural_reference_input::fixed_element_array_view(
            root,
            source,
            result.structural_type,
            &signature.structural_types,
        )
    else {
        return Err(invalid());
    };
    let base = if parameter.is_some() {
        let Some(base) = builder
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == source.place)
            .map(|(_, register)| *register)
        else {
            return Err(invalid());
        };
        base
    } else {
        let mut homes = builder
            .transport
            .local_slots
            .iter()
            .filter(|slot| slot.id.structural_place() == Some(source.place));
        let (Some(home), None) = (homes.next().cloned(), homes.next()) else {
            return Err(invalid());
        };
        address(builder, row, home.id, 0, home.byte_size, false)?
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

/// Present a local aggregate home as the owned root of an element loan. The
/// declaration only supplies window geometry and access; the home's own
/// multiplicity and qualifications still bound what it may lend.
fn local_home_root(
    home: &terminal_psi::StructuralOperationResult,
) -> terminal_psi::StructuralParameterDeclaration {
    terminal_psi::StructuralParameterDeclaration {
        place: home.place,
        position: 0,
        is_self: false,
        structural_type: home.structural_type,
        multiplicity: home.multiplicity,
        access: StructuralAccess::Owned,
        qualifications: home.qualifications.clone(),
        projected_qualifications: home.projected_qualifications.clone(),
    }
}
