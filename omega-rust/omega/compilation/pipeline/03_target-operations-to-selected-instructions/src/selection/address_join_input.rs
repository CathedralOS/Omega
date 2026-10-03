//! Borrowed address joins in legalized input, rejoined for selection.
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
    element_views: &[crate::selection::element_view_homes::ElementViewHomes],
) -> Option<SelectedStructuralTransport> {
    let signature = source.structural.as_ref()?;
    if argument.access != parameter.access
        || !abstract_operations::control_flow::address_joins::is_static_projection(&argument.path)
        || join(source, parameter.place)?.0 != target
    {
        return None;
    }
    let Some(root) = root_type(source, argument.place, argument.access) else {
        return None;
    };
    let Some((referent, byte_offset)) =
        crate::structural_inputs::structural_reference_input::leaf_copy_projection(
            root,
            &argument.path,
            &signature.structural_types,
        )
    else {
        return None;
    };
    let Some(lent) = crate::structural_inputs::structural_reference_input::shape(
        referent,
        &signature.structural_types,
    ) else {
        return None;
    };
    if referent != parameter.structural_type {
        return None;
    }
    let base = if let Some(view) = element_views
        .iter()
        .find(|view| view.place == argument.place)
    {
        // An element view's backing pointer is already live: the join
        // receives the element's address at the projected offset beneath
        // it — not the descriptor's own storage.
        SelectedAddressBase::Register(view.backing_pointer)
    } else {
        storage_base(
            argument.place,
            byte_offset,
            lent.byte_size,
            pointers,
            local_slots,
        )?
    };
    Some(SelectedStructuralTransport::Address {
        base,
        byte_offset,
        byte_count: lent.byte_size,
        destination: LocalStorageSlotId::StructuralBlockParameter {
            block: target,
            place: parameter.place,
        },
    })
}

/// Find the original storage address without reading or copying its contents.
/// Both referent loans and view construction use this; a pointer cell is not
/// implicitly dereferenced here. The caller determines what storage it names.
pub(super) fn storage_base(
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    pointers: &[(PlaceId, VirtualRegisterId)],
    local_slots: &[SelectedLocalStorageSlot],
) -> Option<SelectedAddressBase> {
    if let Some((_, pointer)) = pointers.iter().find(|(candidate, _)| *candidate == place) {
        return Some(SelectedAddressBase::Register(*pointer));
    }
    let mut homes = local_slots.iter().filter(|slot| {
        slot.id.structural_place() == Some(place)
            && !matches!(slot.id, LocalStorageSlotId::StructuralBlockParameter { .. })
    });
    let home = homes.next()?;
    if homes.next().is_some()
        || byte_offset
            .checked_add(byte_count)
            .is_none_or(|end| end > home.byte_size)
    {
        return None;
    }
    Some(SelectedAddressBase::Local(home.id))
}

/// The structural type of a root that may lend an address: a local home, an
/// owned block arrival, an entrance parameter or an earlier address join.
/// Legalization already replayed each root's owner, access and dominance.
pub(super) fn root_type(
    source: &LegalizedScalarFunction,
    place: PlaceId,
    access: StructuralAccess,
) -> Option<StructuralTypeId> {
    if matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) {
        if let Some((_, result)) = crate::selection::aggregate_result_input::home(source, place) {
            return (matches!(
                result.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine
                    | terminal_psi::StructuralMultiplicity::Unrestricted
            ) && result.claims.is_empty()
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty())
            .then_some(result.structural_type);
        }
        if let Some((_, parameter)) = join(source, place) {
            return (parameter.access == StructuralAccess::MutableBorrow
                || parameter.access == access)
                .then_some(parameter.structural_type);
        }
        return source
            .structural
            .as_ref()?
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == place)
            .filter(|parameter| {
                parameter.semantic.access == StructuralAccess::MutableBorrow
                    || parameter.semantic.access == access
            })
            .map(|parameter| parameter.semantic.structural_type);
    }
    if let Some(result) = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|row| match &row.kind {
            legalized_operations::LegalizedScalarInstructionKind::EstablishElementView {
                result,
                ..
            }
            | legalized_operations::LegalizedScalarInstructionKind::ElementViewSubslice {
                result,
                ..
            } => (result.place == place).then_some(result),
            _ => None,
        })
    {
        return Some(result.structural_type);
    }
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
        return (parameter.access != StructuralAccess::WriteOnlyBorrow)
            .then_some(parameter.structural_type);
    }
    let signature = source.structural.as_ref()?;
    // `self` is a valid lending root: its referent record is the attached
    // data, and the projection resolves through that record's field types
    // like any other root's.
    signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == place)
        .filter(|parameter| parameter.semantic.access != StructuralAccess::WriteOnlyBorrow)
        .map(|parameter| parameter.semantic.structural_type)
}

/// The declaration of the loan `place` holds: this unit's own ABI parameter,
/// or the address join of the block that received the storage on an edge.
///
/// A receiver threads one ABI parameter through every state, so a lookup in
/// the signature roster alone answers for the receiver spelling of a program
/// and not for the same program spelled with a named data parameter, where
/// each state holds its own block parameter. The join's block entry already
/// loaded the same referent address the ABI parameter would have supplied, so
/// the two are interchangeable here for either exclusive permission. The caller checks
/// access, multiplicity and qualifications against the returned declaration.
pub(crate) fn loan_declaration(
    source: &LegalizedScalarFunction,
    place: PlaceId,
    access: StructuralAccess,
) -> Option<&StructuralParameterDeclaration> {
    source
        .structural
        .as_ref()?
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .find(|parameter| parameter.place == place)
        .or_else(|| {
            (matches!(
                access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ))
            .then(|| join(source, place))
            .flatten()
            .map(|(_, declared)| declared)
        })
}
