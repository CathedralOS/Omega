//! Replay a shared element-view descriptor over a fixed-array field.
use super::local_storage::{address, store};
use super::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind, PlaceId,
    SelectedInstructionKind, VirtualRegisterId,
};
use crate::SelectedInstructionError;
use crate::selection::validation::scalar_graph::Replay;
use crate::selection::validation::scalar_graph::structural::provenance;
use crate::selection::validation::scalar_graph::structural::result;
use selected_instructions::{LocalStorageSlotId, SelectedLocalStorageSlot};
use semantic_vocabulary::{IntegerValue, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, StructuralTypeShape};

pub(super) fn establish(
    function: &LegalizedScalarFunction,
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
) -> Result<(), SelectedInstructionError> {
    let LegalizedScalarInstructionKind::EstablishElementView {
        result: view,
        destination,
        source,
        element,
    } = &row.kind
    else {
        return Err(replay.invalid());
    };
    if row.result.is_some()
        || view.place != *destination
        // A mutable loan's result carries the exclusive custody's affine
        // multiplicity; the replayed descriptor shape is custody-independent.
        || !matches!(
            view.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        )
        || !view.qualifications.is_empty()
        || !view.projected_qualifications.is_empty()
        || !view.claims.is_empty()
        || !matches!(
            source.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || replay
            .transport
            .pointers
            .iter()
            .any(|(place, _)| *place == *destination)
    {
        return Err(replay.invalid());
    }
    let Some(signature) = function.structural.as_ref() else {
        return Err(replay.invalid());
    };
    let Some(place) = signature
        .structural_places
        .iter()
        .find(|place| place.id == *destination)
    else {
        return Err(replay.invalid());
    };
    if !matches!(
        place.kind,
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } if producer == row.operation && structural_type == view.structural_type
    ) {
        return Err(replay.invalid());
    }
    let Some(declaration) = signature
        .structural_types
        .as_slice()
        .iter()
        .find(|declaration| declaration.id == view.structural_type)
    else {
        return Err(replay.invalid());
    };
    let StructuralTypeShape::ElementView {
        element: view_element,
    } = &declaration.shape
    else {
        return Err(replay.invalid());
    };
    if *view_element != *element {
        return Err(replay.invalid());
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
        return Err(replay.invalid());
    };
    let Some((field_offset, element_count, _stride)) =
        crate::structural_inputs::structural_reference_input::fixed_element_array_view(
            root,
            source,
            view.structural_type,
            signature.structural_types.as_slice(),
        )
    else {
        return Err(replay.invalid());
    };
    let base = if parameter.is_some() {
        let Some(base) = replay
            .transport
            .pointers
            .iter()
            .find(|(place, _)| *place == source.place)
            .map(|(_, register)| *register)
        else {
            return Err(replay.invalid());
        };
        base
    } else {
        let mut homes = replay
            .transport
            .local_slots
            .iter()
            .filter(|slot| slot.id.structural_place() == Some(source.place));
        let (Some(home), None) = (homes.next().cloned(), homes.next()) else {
            return Err(replay.invalid());
        };
        address(replay, row, home.id, 0, home.byte_size, false)?
    };
    let slot = LocalStorageSlotId::Structural {
        operation: row.operation,
        place: *destination,
    };
    replay.transport.local_slots.push(SelectedLocalStorageSlot {
        id: slot,
        byte_size: 16,
        alignment: 8,
    });
    let backing = if field_offset == 0 {
        base
    } else {
        let offset = constant(replay, row, *destination, 0, u64::from(field_offset))?;
        let backing = result(replay, *destination, 0)?;
        replay.check_instruction(
            SelectedInstructionKind::ByteViewAddress,
            replay.constraints.keys.add_i64,
            &[base, offset, backing],
            &provenance(row),
        )?;
        backing
    };
    store(replay, row, slot, 0, backing)?;
    let length = constant(replay, row, *destination, 8, element_count)?;
    store(replay, row, slot, 8, length)?;
    let descriptor = address(replay, row, slot, 0, 16, true)?;
    replay.transport.pointers.push((*destination, descriptor));
    Ok(())
}

fn constant(
    replay: &mut Replay<'_>,
    row: &LegalizedScalarInstruction,
    place: PlaceId,
    offset: u32,
    value: u64,
) -> Result<VirtualRegisterId, SelectedInstructionError> {
    let register = result(replay, place, offset)?;
    replay.check_instruction(
        SelectedInstructionKind::MaterializeI64 {
            value: IntegerValue::Unsigned(u128::from(value)),
        },
        replay.constraints.keys.materialize_i64,
        &[register],
        &provenance(row),
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
