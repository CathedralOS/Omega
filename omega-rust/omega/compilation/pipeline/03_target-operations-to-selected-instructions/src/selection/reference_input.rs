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
    /// A projection selects the root's storage payload, not its whole carrier.
    pub(super) projects_root_storage: bool,
    pub(super) byte_offset: u32,
    /// A trailing Referent on a reference leaf reads that leaf's stored word.
    /// Otherwise the projected storage address itself is the borrowed value.
    pub(super) load_pointer: bool,
    /// Scalar descriptor sources preserve the stored codec on reborrow.
    pub(super) load_codec: bool,
    pub(super) codec: Option<runtime_abi::ScalarStorageCodec>,
    pub(super) storage: Option<terminal_psi::ReferenceStorageView>,
    pub(super) descriptor_offsets: Vec<(u32, std::ops::Range<usize>)>,
    pub(super) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
    pub(super) leaf_index: usize,
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
    let LegalizedScalarInstructionKind::EstablishReference {
        result,
        source,
        storage,
        indices,
        shape,
    } = &row.kind
    else {
        return None;
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
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        root,
        path,
        declarations,
    )?;
    if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
        indices,
        &geometry.elements,
    ) {
        return None;
    }
    let leaf_index = geometry
        .pointer_slots
        .last()
        .map_or(0, |slot| slot.indices.end);
    let descriptor_offsets = geometry
        .pointer_slots
        .into_iter()
        .map(|slot| (slot.byte_offset, slot.indices))
        .collect();
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
    let load_codec = storage.is_none()
        && semantic_scalar.is_some()
        && (load_pointer || borrowed_scalar_source(function, source.place).is_some());
    load_pointer |= load_codec;
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
    (match storage {
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
    })
    .then_some(ReferenceAddress {
        place: source.place,
        projects_root_storage: !path.is_empty(),
        byte_offset,
        load_pointer,
        load_codec,
        codec,
        storage: *storage,
        descriptor_offsets,
        indices: indices.clone(),
        leaf_index,
    })
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
    super::aggregate_result_input::home(function, place).map(|(_, result)| result.structural_type)
}

/// Cross only the explicit referent boundary of an exact completed mutable
/// carrier. The retained path stays on the operation; this borrowed suffix is
/// used solely to reconstruct geometry within the selected referent.
pub(super) fn mutable_projection<'path>(
    function: &LegalizedScalarFunction,
    place: PlaceId,
    path: &'path [StructuralPathSegment],
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let (StructuralPathSegment::Referent, suffix) = path.split_first()? else {
        return None;
    };
    // The storage query admits the complete suffix. Selector presence does
    // not change the carrier's required pointer crossing.
    let structural_type =
        if let Some((_, result)) = super::aggregate_result_input::home(function, place) {
            carrier_referent(function, result)?;
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
    let StructuralTypeShape::Reference { referent, access } = terminal_psi::find_structural_type(
        &function.structural.as_ref()?.structural_types,
        structural_type,
    )?
    .shape
    else {
        return None;
    };
    (access == StructuralAccess::MutableBorrow).then_some((referent, suffix))
}

pub(super) fn store_destination<'path>(
    function: &LegalizedScalarFunction,
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &'path [StructuralPathSegment],
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
    mutable_projection(function, destination.place(), path)
}

/// An outgoing borrow transports the pointer word, not the carrier home's
/// address. Exact target metadata must retain the authored carrier and path.
/// Reconstruct the address independently of the destination ABI. Internal
/// calls validate their descriptor placement; normalized foreign calls validate
/// their evaluated pointer placement and reject scalar codec descriptors.
pub(super) fn borrowed_argument(
    function: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
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
            {
                ValueShape::borrowed_scalar_reference()
            } else {
                ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
            }
        && target.indices.is_empty()
        && target.fixed_array_length.is_none()
        && target.element_stride.is_none())
    .then_some(BorrowedReferenceAddress {
        descriptor_offsets: projection.descriptor_offsets,
        referent_offset: offset,
        scalar_descriptor: projection.scalar_descriptor,
    })
}

pub(super) struct BorrowedReferenceAddress {
    pub(super) descriptor_offsets: Vec<u32>,
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
