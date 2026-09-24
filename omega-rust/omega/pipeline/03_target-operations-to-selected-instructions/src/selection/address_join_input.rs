//! Shared address joins in legalized input, rejoined for selection.
//!
//! An address join (`abstract_operations::control_flow::address_joins`) owns
//! one 8-byte block slot holding its referent's address. Its block entry
//! loads that address once, so every later consumer sees the same pointer an
//! incoming borrowed parameter would supply. Each edge into the join lends a
//! root's storage: a pointer already live in the source block, or a
//! frame-resident home addressed on the edge. Construction and validation
//! both derive the edge transport here from the legalized function and their
//! own transport state; neither trusts the other's choice.

use calling_conventions::ValueShape;
use legalized_operations::LegalizedScalarFunction;
use selected_instructions::{
    LocalStorageSlotId, SelectedAddressBase, SelectedLocalStorageSlot, SelectedStructuralTransport,
    VirtualRegisterId,
};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralParameterDeclaration};

/// The address join declared at `place`, with its owning block.
pub(crate) fn join(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<(BlockId, &StructuralParameterDeclaration)> {
    let signature = source.structural.as_ref()?;
    let mut matching = source.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, parameter) = matching.next()?;
    (matching.next().is_none()
        && block != source.entry_block
        && abstract_operations::control_flow::address_joins::is_address_join_in(
            parameter,
            &signature.structural_types,
        )
        && signature.structural_places.iter().any(|declaration| {
            declaration.id == place
                && declaration.kind
                    == (semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                        block,
                        position: parameter.position,
                    })
        }))
    .then_some((block, parameter))
}

/// The join's block-owned carrier: exactly one pointer.
pub(crate) fn carrier_shape() -> ValueShape {
    ValueShape::integer(8, 8)
}

/// The borrowed-reference shape a call receives for the joined referent.
pub(crate) fn argument_shape(
    source: &LegalizedScalarFunction,
    parameter: &StructuralParameterDeclaration,
) -> Option<ValueShape> {
    let signature = source.structural.as_ref()?;
    let referent = crate::structural_inputs::structural_reference_input::shape(
        parameter.structural_type,
        &signature.structural_types,
    )?;
    Some(ValueShape::borrowed_reference(
        referent.byte_size,
        referent.alignment,
    ))
}

/// The address transport binding `argument` into the join `parameter` of
/// `target`. `pointers` are the source block's live place pointers and
/// `local_slots` its declared frame homes.
pub(crate) fn transport(
    source: &LegalizedScalarFunction,
    target: BlockId,
    parameter: &StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    pointers: &[(PlaceId, VirtualRegisterId)],
    local_slots: &[SelectedLocalStorageSlot],
) -> Option<SelectedStructuralTransport> {
    let signature = source.structural.as_ref()?;
    if argument.access != StructuralAccess::SharedBorrow
        || !abstract_operations::control_flow::address_joins::is_static_projection(&argument.path)
        || join(source, parameter.place)?.0 != target
    {
        return None;
    }
    let (referent, byte_offset) = crate::structural_inputs::structural_reference_input::project(
        root_type(source, argument.place)?,
        &argument.path,
        &signature.structural_types,
    )?;
    let lent = crate::structural_inputs::structural_reference_input::shape(
        referent,
        &signature.structural_types,
    )?;
    if referent != parameter.structural_type {
        return None;
    }
    let base =
        if let Some((_, pointer)) = pointers.iter().find(|(place, _)| *place == argument.place) {
            SelectedAddressBase::Register(*pointer)
        } else {
            let mut homes = local_slots.iter().filter(|slot| {
                slot.id.structural_place() == Some(argument.place)
                    && !matches!(slot.id, LocalStorageSlotId::StructuralBlockParameter { .. })
            });
            let home = homes.next()?;
            if homes.next().is_some()
                || byte_offset
                    .checked_add(u32::from(lent.byte_size))
                    .is_none_or(|end| end > home.byte_size)
            {
                return None;
            }
            SelectedAddressBase::Local(home.id)
        };
    Some(SelectedStructuralTransport::Address {
        base,
        byte_offset,
        byte_count: u32::from(lent.byte_size),
        destination: LocalStorageSlotId::StructuralBlockParameter {
            block: target,
            place: parameter.place,
        },
    })
}

/// The structural type of a root that may lend an address: a local home, an
/// owned block arrival, an entrance parameter or an earlier address join.
/// Legalization already replayed each root's owner, access and dominance.
fn root_type(source: &LegalizedScalarFunction, place: PlaceId) -> Option<StructuralTypeId> {
    if let Some((_, result)) = crate::selection::aggregate_result_input::home(source, place) {
        return Some(result.structural_type);
    }
    if let Some((_, result, _, _)) = crate::selection::primitive_local_input::local(source, place) {
        return Some(result.structural_type);
    }
    if let Some((_, parameter)) =
        crate::selection::aggregate_result_input::block_home(source, place)
    {
        return Some(parameter.structural_type);
    }
    if let Some((_, parameter)) = join(source, place) {
        return Some(parameter.structural_type);
    }
    let signature = source.structural.as_ref()?;
    signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == place)
        .filter(|parameter| {
            !parameter.semantic.is_self
                && parameter.semantic.access != StructuralAccess::WriteOnlyBorrow
        })
        .map(|parameter| parameter.semantic.structural_type)
}
