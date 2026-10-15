//! Borrowed address joins in legalized input, rejoined for selection.
//!
//! An address join (`abstract_operations::control_flow::address_joins`) owns
//! a block slot holding its referent's address, plus a codec for scalar loans.
//! Its block entry retains the same carrier an incoming borrowed parameter
//! would supply. Each edge into the join lends a
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

/// The join's block-owned carrier: a thin aggregate pointer or scalar descriptor.
pub(crate) fn carrier_shape(
    source: &LegalizedScalarFunction,
    parameter: &StructuralParameterDeclaration,
) -> ValueShape {
    let scalar = source.structural.as_ref().and_then(|signature| {
        crate::structural_inputs::structural_reference_input::scalar_storage_type(
            parameter.structural_type,
            &signature.structural_types,
        )
    });
    ValueShape::integer(if scalar.is_some() { 16 } else { 8 }, 8)
}

/// The address transport binding `argument` into the join `parameter` of
/// `target`. `pointers` are the source block's live place pointers and
/// `local_slots` its declared frame homes.
pub(crate) fn projection(
    source: &LegalizedScalarFunction,
    target: BlockId,
    parameter: &StructuralParameterDeclaration,
    argument: &terminal_psi::StructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
    pointers: &[(PlaceId, VirtualRegisterId)],
    local_slots: &[SelectedLocalStorageSlot],
    element_views: &[crate::selection::element_view_homes::ElementViewHomes],
    byte_views: &[crate::selection::byte_view_homes::ByteViewHomes],
) -> Option<super::aggregate_result_input::BlockParameterProjection> {
    let signature = source.structural.as_ref()?;
    if argument.access != parameter.access
        || !abstract_operations::control_flow::address_joins::is_address_projection(&argument.path)
        || join(source, parameter.place)?.0 != target
    {
        return None;
    }
    // A whole bounded owner loan lends its completed owner's address whatever
    // byte predicates that owner carries, as legalization's owner-source replay
    // admits; the checked abstract unit, not selection, owns predicate admission.
    let owner_home = || {
        let (_, result) = crate::selection::aggregate_result_input::home(source, argument.place)?;
        (argument.path.is_empty()
            && abstract_operations::control_flow::owner_loans::is_bounded_owner_loan_in(
                parameter,
                &signature.structural_types,
            )
            && result.structural_type == parameter.structural_type
            && result.claims.is_empty()
            && result.multiplicity != terminal_psi::StructuralMultiplicity::Linear
            && result.projected_qualifications.is_empty())
        .then_some(result.structural_type)
    };
    let Some(root) =
        root_type(source, argument.place, argument.access, &argument.path).or_else(owner_home)
    else {
        return None;
    };
    let supplied = super::reference_input::source_access(source, argument.place)?;
    if supplied.structural_type != root
        || !terminal_semantics::reference_source_projection_accessible(
            signature.structural_types.iter(),
            root,
            supplied.multiplicity,
            supplied.access,
            &argument.path,
            argument.access,
        )
    {
        return None;
    }
    // Rejoin the selected endpoint and every stored pointer's permission from
    // this consumer's catalog before deriving runtime descriptor loads.
    if abstract_operations::control_flow::address_joins::borrowed_referent_type(
        signature.structural_types.iter(),
        root,
        &argument.path,
        argument.access,
    ) != Some(parameter.structural_type)
    {
        return None;
    }
    let mut geometry =
        crate::structural_inputs::structural_reference_input::reference_source_geometry(
            root,
            &argument.path,
            &signature.structural_types,
        )?;
    if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
        indices,
        &geometry.elements,
    ) {
        return None;
    }
    let referent = geometry.selected;
    // Scalar loans retain the descriptor home, not just its pointer word.
    // Snapshotting below transports the actual address and codec together.
    let retained_scalar_descriptor = geometry
        .pointer_slots
        .last()
        .is_some_and(|slot| slot.scalar_descriptor);
    let byte_offset = if retained_scalar_descriptor {
        if argument.path.last() != Some(&terminal_psi::StructuralPathSegment::Referent) {
            return None;
        }
        geometry.pointer_slots.pop()?.byte_offset
    } else {
        geometry.byte_offset
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
    let scalar = crate::structural_inputs::structural_reference_input::scalar_storage_type(
        referent,
        &signature.structural_types,
    );
    let descriptor = retained_scalar_descriptor
        || argument.path.is_empty()
            && super::reference_input::borrowed_scalar_source(source, argument.place).is_some();
    let codec = scalar.and_then(|_| {
        crate::structural_inputs::structural_reference_input::projected_scalar_codec(
            root,
            &argument.path,
            &signature.structural_types,
        )
    });
    let storage_offset = geometry
        .pointer_slots
        .first()
        .map_or(byte_offset, |slot| slot.byte_offset);
    let storage_bytes = if !geometry.pointer_slots.is_empty() {
        8
    } else if descriptor {
        16
    } else if let Some(codec) = codec {
        u32::from(codec.byte_size())
    } else {
        lent.byte_size
    };
    let (base, base_byte_offset) = projection_base(
        argument.place,
        storage_offset,
        storage_bytes,
        &mut geometry.pointer_slots,
        pointers,
        local_slots,
        element_views,
        byte_views,
    )?;
    if scalar.is_some() {
        let source = if descriptor {
            selected_instructions::SelectedScalarReferenceSource::Descriptor
        } else {
            selected_instructions::SelectedScalarReferenceSource::Canonical {
                codec: codec?.encode()?,
            }
        };
        return Some(super::aggregate_result_input::BlockParameterProjection {
            transport: SelectedStructuralTransport::ScalarReference {
                base,
                byte_offset,
                source,
                destination: LocalStorageSlotId::StructuralBlockParameter {
                    block: target,
                    place: parameter.place,
                },
            },
            pointer_slots: geometry.pointer_slots,
            indices: indices.to_vec(),
            base_byte_offset,
        });
    }
    Some(super::aggregate_result_input::BlockParameterProjection {
        transport: SelectedStructuralTransport::Address {
            base,
            byte_offset,
            byte_count: lent.byte_size,
            destination: LocalStorageSlotId::StructuralBlockParameter {
                block: target,
                place: parameter.place,
            },
        },
        pointer_slots: geometry.pointer_slots,
        indices: indices.to_vec(),
        base_byte_offset,
    })
}

/// A projected view starts at its live backing coordinate, whereas a whole
/// descriptor starts at its home. Only the initial backing crossing is already
/// realized by the SSA view; nested descriptor loads retain their own regions.
pub(super) fn projection_base(
    place: PlaceId,
    byte_offset: u32,
    byte_count: u32,
    pointer_slots: &mut Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
    pointers: &[(PlaceId, VirtualRegisterId)],
    local_slots: &[SelectedLocalStorageSlot],
    element_views: &[super::element_view_homes::ElementViewHomes],
    byte_views: &[super::byte_view_homes::ByteViewHomes],
) -> Option<(SelectedAddressBase, Option<VirtualRegisterId>)> {
    let view = element_views
        .iter()
        .find(|view| view.place == place)
        .map(|view| (view.backing_pointer, view.byte_offset))
        .or_else(|| {
            byte_views
                .iter()
                .find(|view| view.place == place)
                .map(|view| (view.backing_pointer, view.byte_offset))
        });
    if !pointer_slots.is_empty()
        && let Some((pointer, offset)) = view
    {
        let first = pointer_slots.first()?;
        if first.byte_offset != 0 || first.indices != (0..0) || first.scalar_descriptor {
            return None;
        }
        pointer_slots.remove(0);
        return Some((SelectedAddressBase::Register(pointer), Some(offset)));
    }
    storage_base(place, byte_offset, byte_count, pointers, local_slots).map(|base| (base, None))
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
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    if matches!(access, terminal_psi::exclusive_borrow_pair_pattern!()) {
        let permits = |parameter: &StructuralParameterDeclaration| {
            abstract_operations::control_flow::address_joins::loan_access_permitted(
                source
                    .structural
                    .as_ref()
                    .into_iter()
                    .flat_map(|signature| signature.structural_types.iter()),
                parameter.structural_type,
                path,
                parameter.access,
                access,
            )
        };
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
            return permits(parameter).then_some(parameter.structural_type);
        }
        if let Some((_, parameter)) =
            crate::selection::aggregate_result_input::block_home(source, place)
        {
            return permits(parameter).then_some(parameter.structural_type);
        }
        return source
            .structural
            .as_ref()?
            .parameters
            .iter()
            .find(|parameter| parameter.semantic.place == place)
            .filter(|parameter| permits(&parameter.semantic))
            .map(|parameter| parameter.semantic.structural_type);
    }
    if let Some(result) = source.instructions().find_map(|row| match &row.kind {
        legalized_operations::LegalizedScalarInstructionKind::EstablishElementView {
            result,
            ..
        }
        | legalized_operations::LegalizedScalarInstructionKind::ElementViewSubslice {
            result,
            ..
        } => (result.place == place).then_some(result),
        _ => None,
    }) {
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
            (matches!(access, terminal_psi::exclusive_borrow_pair_pattern!()))
                .then(|| join(source, place))
                .flatten()
                .map(|(_, declared)| declared)
        })
}
