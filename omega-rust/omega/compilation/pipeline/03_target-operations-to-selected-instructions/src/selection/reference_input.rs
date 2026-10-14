//! Physical reference establishment rejoins exact legalized source geometry.
//!
//! The carrier stores an address, never the referent's bytes. Source legality,
//! live loans and result origins remain independently replayed by legalization;
//! this owner reconstructs which address operation selection must realize.

use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

pub(super) struct ReferenceAddress {
    pub(super) place: PlaceId,
    pub(super) byte_offset: u32,
    /// A trailing Referent on a reference leaf reads that leaf's stored word.
    /// Otherwise the projected storage address itself is the borrowed value.
    pub(super) load_pointer: bool,
    /// Scalar descriptor sources preserve the stored codec on reborrow.
    pub(super) load_codec: bool,
    pub(super) codec: Option<runtime_abi::ScalarStorageCodec>,
    pub(super) storage: Option<terminal_psi::ReferenceStorageView>,
    pub(super) placed: Option<terminal_psi::PlacedRamCorrespondence>,
    pub(super) pointer_slots:
        Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
    pub(super) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
}

pub(in crate::selection) fn borrowed_scalar_source(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::ScalarType> {
    let signature = function.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == place)
        .map(|parameter| &parameter.semantic)
        .or_else(|| {
            function
                .block_structural_parameters()
                .find(|parameter| parameter.place == place)
        })?;
    (parameter.access != StructuralAccess::Owned).then_some(())?;
    crate::structural_inputs::structural_reference_input::scalar_storage_type(
        parameter.structural_type,
        &signature.structural_types,
    )
}

pub(super) fn establishment(
    function: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<ReferenceAddress> {
    let (result, source, storage, indices, shape, placed) = match &row.kind {
        LegalizedScalarInstructionKind::EstablishReference {
            result,
            source,
            storage,
            indices,
            shape,
        } => (result, source, *storage, indices, shape, None),
        LegalizedScalarInstructionKind::EstablishPlacedReference {
            result,
            source,
            correspondence,
            indices,
            shape,
        } => (result, source, None, indices, shape, Some(correspondence)),
        _ => return None,
    };
    let signature = function.structural.as_ref()?;
    let declarations = &signature.structural_types;
    if row.result.is_some()
        || Some(*shape)
            != crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                declarations,
            )
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
        || super::aggregate_result_input::home(function, result.place)
            != Some((row.operation, result))
    {
        return None;
    }
    let StructuralTypeShape::Reference { access, referent } = declarations
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?
        .shape
    else {
        return None;
    };
    // Formation captures an address; it does not read the lent bytes. A
    // write-only loan therefore forms the same descriptor while retaining
    // its exact access. Only mutable access can attenuate to another kind.
    if source.access == StructuralAccess::Owned
        || !(access == source.access
            || source.access == StructuralAccess::MutableBorrow
                && matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
    {
        return None;
    }
    let root = root_type(function, source.place)?;
    let semantic_scalar = crate::structural_inputs::structural_reference_input::scalar_storage_type(
        referent,
        declarations,
    );
    let (path, through_referent) = match source.path.split_last() {
        Some((StructuralPathSegment::Referent, prefix)) => (prefix, true),
        _ => (source.path.as_slice(), false),
    };
    let mut projection_root = root;
    let mut segment_start = 0;
    let geometry = crate::structural_inputs::structural_reference_input::borrowed_source_geometry(
        root,
        path,
        declarations,
        source.access,
    )?;
    if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
        indices,
        &geometry.elements,
    ) {
        return None;
    }
    for (position, segment) in path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let (carrier, _, _) =
            crate::structural_inputs::structural_reference_input::runtime_projection(
                projection_root,
                &path[segment_start..position],
                declarations,
            )?;
        let StructuralTypeShape::Reference { referent, .. } = declarations
            .iter()
            .find(|declaration| declaration.id == carrier)?
            .shape
        else {
            return None;
        };
        projection_root = referent;
        segment_start = position + 1;
    }
    let projection_path = &path[segment_start..];
    let selected = geometry.selected;
    let byte_offset = geometry.byte_offset;
    let selected_shape = &declarations
        .iter()
        .find(|declaration| declaration.id == selected)?
        .shape;
    let (actual, mut load_pointer) = match selected_shape {
        StructuralTypeShape::Reference { referent, .. } if through_referent => (*referent, true),
        _ => (selected, false),
    };
    // Borrowed scalar ingress carries an address-and-codec descriptor. A new
    // interpretation replaces the codec, not the descriptor's pointer crossing.
    load_pointer |= borrowed_scalar_source(function, source.place).is_some();
    let load_codec = storage.is_none() && semantic_scalar.is_some() && load_pointer;
    let codec = match storage {
        Some(storage) => storage
            .stored_scalar
            .and_then(runtime_abi::ScalarStorageCodec::from_scalar),
        None if !load_codec && semantic_scalar.is_some() => {
            crate::structural_inputs::structural_reference_input::projected_scalar_codec(
                projection_root,
                projection_path,
                declarations,
            )
        }
        None => None,
    };
    (if let Some(correspondence) = placed {
        let mut plans = declarations
            .placed_views()
            .iter()
            .filter(|plan| plan.carrier == referent);
        match (plans.next(), plans.next()) {
            (Some(plan), None) => {
                actual == correspondence.schema
                    && terminal_semantics::placed_ram_plan_correspondence(
                        declarations.as_slice(),
                        declarations.placed_views(),
                        plan,
                        actual,
                    )
                    .is_ok_and(|expected| &expected == correspondence)
            }
            _ => false,
        }
    } else {
        match storage {
            Some(storage) => terminal_semantics::reference_storage_source_compatible(
                declarations.iter(),
                actual,
                result.structural_type,
                storage.stored_scalar,
            ),
            None => terminal_semantics::reference_source_compatible(
                declarations.iter(),
                actual,
                result.structural_type,
            ),
        }
    })
    .then_some(ReferenceAddress {
        place: source.place,
        byte_offset,
        load_pointer,
        load_codec,
        codec,
        storage,
        placed: placed.cloned(),
        pointer_slots: geometry.pointer_slots,
        indices: indices.clone(),
    })
    .and_then(|address| {
        if address.placed.is_some() && address.load_pointer {
            let correspondence = address.placed.clone();
            let mut actual =
                local_reference_frame_address(function, row, address, &mut Vec::new())?;
            actual.placed = correspondence;
            Some(actual)
        } else {
            Some(address)
        }
    })
}

/// Rejoin a whole ordinary descriptor to its exact immutable establishment.
/// The validated legalized custody already proves this definition is live at
/// the factory. Incoming pointers, stored crossings, storage re-views and
/// projected descriptor values do not establish a native frame base.
fn local_reference_frame_address(
    function: &LegalizedScalarFunction,
    consumer: &LegalizedScalarInstruction,
    address: ReferenceAddress,
    pending: &mut Vec<PlaceId>,
) -> Option<ReferenceAddress> {
    if !address.load_pointer {
        return Some(address);
    }
    if address.load_codec
        || address.storage.is_some()
        || address.byte_offset != 0
        || !address.pointer_slots.is_empty()
        || !address.indices.is_empty()
        || pending.contains(&address.place)
    {
        return None;
    }
    pending.push(address.place);
    let (definition, result) = super::aggregate_result_input::home(function, address.place)?;
    let mut rows = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|row| row.operation == definition);
    let (Some(row), None) = (rows.next(), rows.next()) else {
        return None;
    };
    let LegalizedScalarInstructionKind::EstablishReference {
        result: retained,
        storage: None,
        ..
    } = &row.kind
    else {
        return None;
    };
    if retained != result || retained.place != address.place {
        return None;
    }
    // An adjacent same-block definition cannot have been reseated, loaned
    // or modified by a call before capture. A declaration/home alone does not
    // prove that the current descriptor still stores its original address.
    if !function.blocks.iter().any(|block| {
        block.instructions.windows(2).any(|pair| {
            pair[0].operation == row.operation && pair[1].operation == consumer.operation
        })
    }) {
        return None;
    }
    let prior = establishment(function, row)?;
    if prior.placed.is_some() || prior.storage.is_some() {
        return None;
    }
    let resolved = local_reference_frame_address(function, row, prior, pending);
    pending.pop();
    resolved
}

pub(super) fn root_type(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = function.structural.as_ref()?;
    let mut parameters = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(
            function
                .blocks
                .iter()
                .filter(|block| block.id != function.entry_block)
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| parameter.place == place);
    if let Some(parameter) = parameters.next() {
        return parameters
            .next()
            .is_none()
            .then_some(parameter.structural_type);
    }
    if let Some((_, result, _, _)) = super::primitive_local_input::local(function, place) {
        return Some(result.structural_type);
    }
    super::aggregate_result_input::home(function, place)
        .map(|(_, result)| result.structural_type)
        // Literal descriptors use their authenticated immutable establishment,
        // rather than an operation-result allocation declaration.
        .or_else(|| {
            let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                structural_type,
                ..
            } = signature.structural_place(place)?.kind
            else {
                return None;
            };
            super::established_view_input::view_access(function, place)?;
            (super::established_view_input::view_type(function, place) == Some(structural_type))
                .then_some(structural_type)
        })
}

/// Cross only the explicit referent boundary of an exact completed carrier
/// with the operation's required access. The retained path stays on the operation; this borrowed suffix is
/// used solely to reconstruct geometry within the selected referent.
pub(super) fn referent_projection<'path>(
    function: &LegalizedScalarFunction,
    place: PlaceId,
    path: &'path [StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let structural_type =
        if let Some((_, result)) = super::aggregate_result_input::home(function, place) {
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.qualification_establishments.is_empty()
                || !result.claims.is_empty()
            {
                return None;
            }
            result.structural_type
        } else {
            let (_, parameter) = super::aggregate_result_input::block_home(function, place)?;
            if parameter.multiplicity != StructuralMultiplicity::Affine
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
            parameter.structural_type
        };
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        structural_type,
        &path[..=crossing],
        required_access,
        &function.structural.as_ref()?.structural_types,
    )?;
    Some((projection.selected, &path[crossing + 1..]))
}

pub(super) fn store_destination<'path>(
    function: &LegalizedScalarFunction,
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &'path [StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let admitted = match destination {
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            super::aggregate_result_input::home(function, result.place) == Some((*producer, result))
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            block,
            parameter,
        } => {
            super::aggregate_result_input::block_home(function, parameter.place)
                == Some((*block, parameter))
        }
        abstract_operations::StructuralScalarStoreDestination::Parameter(_) => false,
    };
    if !admitted {
        return None;
    }
    referent_projection(function, destination.place(), path, required_access)
}

/// An outgoing borrow transports the pointer word, not the carrier home's
/// address. Exact target metadata must retain the authored carrier and path.
/// Reconstruct the address independently of the destination ABI. Internal
/// calls validate their descriptor placement; normalized foreign calls validate
/// their evaluated thin-pointer placement and prove the exported footprint
/// before discarding a scalar codec descriptor.
pub(super) fn borrowed_argument(
    function: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Option<BorrowedReferenceAddress> {
    let root = root_type(function, argument.place)?;
    let signature = function.structural.as_ref()?;
    let declarations = &signature.structural_types;
    match &target.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            let (producer, result) = super::aggregate_result_input::home(function, argument.place)?;
            if producer != *psi_operation || result.structural_type != root {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            let parameter = signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == argument.place)?;
            if parameter.target.placement != *placement
                || parameter.semantic.structural_type != root
                || parameter.semantic.access == StructuralAccess::WriteOnlyBorrow
            {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
            let (owner, parameter) =
                super::aggregate_result_input::block_home(function, argument.place)?;
            if owner != *block || *place != argument.place || parameter.structural_type != root {
                return None;
            }
        }
        _ => return None,
    }
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        root,
        &argument.path,
        argument.access,
        declarations,
    )?;
    let selected = projection.selected;
    let offset = projection.referent_offset;
    let shape =
        crate::structural_inputs::structural_reference_input::shape(selected, declarations)?;
    (target.place == argument.place
        && target.path == argument.path
        && target.access == argument.access
        && target.root_structural_type == root
        && target.structural_type == selected
        && target.source_byte_offset == offset
        && target.shape
            == if crate::structural_inputs::structural_reference_input::scalar_storage_type(
                selected,
                declarations,
            )
            .is_some()
                && target.destination.shape.class
                    == calling_conventions::ValueClass::BorrowedScalarReference
            {
                ValueShape::borrowed_scalar_reference()
            } else {
                ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
            }
        && crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices,
            &projection.elements,
        )
        && target.fixed_array_length.is_none()
        && target.element_stride.is_none())
    .then_some(BorrowedReferenceAddress {
        pointer_slots: projection.pointer_slots,
        indices: indices.to_vec(),
        referent_offset: offset,
        scalar_descriptor: projection.scalar_descriptor,
    })
}

pub(super) struct BorrowedReferenceAddress {
    pub(super) pointer_slots:
        Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
    pub(super) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
    pub(super) referent_offset: u32,
    pub(super) scalar_descriptor: bool,
}

fn carrier_referent(
    function: &LegalizedScalarFunction,
    result: &terminal_psi::StructuralOperationResult,
) -> Option<(StructuralTypeId, StructuralAccess)> {
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
    {
        return None;
    }
    match function
        .structural
        .as_ref()?
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?
        .shape
    {
        StructuralTypeShape::Reference { referent, access } => Some((referent, access)),
        _ => None,
    }
}
