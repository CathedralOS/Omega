//! Establish an element-view descriptor over resolved fixed-array storage.
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
    match &declaration.shape {
        StructuralTypeShape::ElementView {
            element: view_element,
        } if *view_element == *element => {}
        // A `&[u8]` result declares its view over the `u8` element instead of
        // an `ElementView` shell; the descriptor shape is identical.
        StructuralTypeShape::ByteSequence(_)
            if declaration.shape.is_borrowed_byte_view()
                && signature
                    .structural_types
                    .iter()
                    .find(|element_declaration| element_declaration.id == *element)
                    .is_some_and(|element_declaration| {
                        matches!(
                            element_declaration.shape,
                            StructuralTypeShape::PrimitiveScalar(
                                semantic_vocabulary::ScalarType::Integer(integer)
                            ) if Ok(integer)
                                == semantic_vocabulary::IntegerType::new(
                                    semantic_vocabulary::IntegerSign::Unsigned,
                                    8,
                                )
                        )
                    }) => {}
        _ => return Err(invalid()),
    }
    // Incoming parameters lend their original address; operation results and
    // owned block arrivals lend their exact activation slot. A transferred
    // descriptor is not an owned aggregate and cannot supply this backing.
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == source.place)
        .map(|parameter| &parameter.semantic);
    let home = crate::selection::aggregate_result_input::home(function, source.place);
    let local_root = home.map(|(_, home)| local_home_root(home));
    let block_home = crate::selection::aggregate_result_input::block_home(function, source.place);
    let Some(root) = parameter
        .or(local_root.as_ref())
        .or(block_home.map(|(_, declaration)| declaration))
    else {
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
    // An empty view has no dereferenceable payload. Keep its independently
    // checked backing identity, but do not require a slot for a zero-byte
    // array or invent dummy storage. Its descriptor is the empty (0, 0) view.
    let base = if element_count == 0 {
        constant(builder, row, 0, 0)?
    } else if parameter.is_some() {
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
        let mut homes = builder.transport.local_slots.iter().filter(|slot| {
            Some(slot.id)
                == home
                    .map(|(operation, _)| LocalStorageSlotId::Structural {
                        operation,
                        place: source.place,
                    })
                    .or_else(|| {
                        block_home.map(|(block, _)| LocalStorageSlotId::StructuralBlockParameter {
                            block,
                            place: source.place,
                        })
                    })
        });
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
    let backing = if field_offset == 0 || element_count == 0 {
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
