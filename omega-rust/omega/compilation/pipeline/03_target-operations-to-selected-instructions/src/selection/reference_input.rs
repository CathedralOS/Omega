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
}

pub(super) fn establishment(
    function: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<ReferenceAddress> {
    let LegalizedScalarInstructionKind::EstablishReference {
        result,
        source,
        shape,
    } = &row.kind
    else {
        return None;
    };
    let signature = function.structural.as_ref()?;
    let declarations = &signature.structural_types;
    if row.result.is_some()
        || *shape != ValueShape::integer(8, 8)
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
    let StructuralTypeShape::Reference { access, .. } = declarations
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?
        .shape
    else {
        return None;
    };
    if !matches!(
        source.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !(access == source.access
        || access == StructuralAccess::SharedBorrow
            && source.access == StructuralAccess::MutableBorrow)
    {
        return None;
    }
    let root = root_type(function, source.place)?;
    let (path, through_referent) = match source.path.split_last() {
        Some((StructuralPathSegment::Referent, prefix)) => (prefix, true),
        _ => (source.path.as_slice(), false),
    };
    let (selected, byte_offset) =
        crate::structural_inputs::structural_reference_input::project(root, path, declarations)?;
    let selected_shape = &declarations
        .iter()
        .find(|declaration| declaration.id == selected)?
        .shape;
    let (actual, load_pointer) = match selected_shape {
        StructuralTypeShape::Reference { referent, .. } if through_referent => (*referent, true),
        _ => (selected, false),
    };
    terminal_semantics::reference_source_compatible(
        declarations.iter(),
        actual,
        result.structural_type,
    )
    .then_some(ReferenceAddress {
        place: source.place,
        byte_offset,
        load_pointer,
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
    if suffix.iter().any(|segment| {
        !matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    let (_, result) = super::aggregate_result_input::home(function, place)?;
    let (referent, access) = carrier_referent(function, result)?;
    (access == StructuralAccess::MutableBorrow).then_some((referent, suffix))
}

pub(super) fn store_destination<'path>(
    function: &LegalizedScalarFunction,
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &'path [StructuralPathSegment],
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } =
        destination
    else {
        return None;
    };
    if super::aggregate_result_input::home(function, result.place) != Some((*producer, result)) {
        return None;
    }
    mutable_projection(function, result.place, path)
}

/// An outgoing borrow transports the pointer word, not the carrier home's
/// address. Exact target metadata must retain the authored carrier and path.
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
    let crossing = argument
        .path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::Referent))?;
    if argument.path.iter().enumerate().any(|(position, segment)| {
        position != crossing
            && !matches!(
                segment,
                StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
            )
    }) {
        return None;
    }
    let (prefix, crossing_and_suffix) = argument.path.split_at(crossing);
    let suffix = &crossing_and_suffix[1..];
    let (carrier, descriptor_offset) =
        crate::structural_inputs::structural_reference_input::project(root, prefix, declarations)?;
    let StructuralTypeShape::Reference { referent, access } = declarations
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let (selected, offset) = crate::structural_inputs::structural_reference_input::project(
        referent,
        suffix,
        declarations,
    )?;
    let shape =
        crate::structural_inputs::structural_reference_input::shape(selected, declarations)?;
    let allowed_access = match access {
        StructuralAccess::MutableBorrow => matches!(
            argument.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
        _ => false,
    };
    (allowed_access
        && target.place == argument.place
        && target.path == argument.path
        && target.access == argument.access
        && target.root_structural_type == root
        && target.structural_type == selected
        && target.source_byte_offset == offset
        && target.shape == ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
        && target.destination.shape == target.shape
        && target.indices.is_empty()
        && target.fixed_array_length.is_none()
        && target.element_stride.is_none())
    .then_some(BorrowedReferenceAddress {
        descriptor_offset,
        referent_offset: offset,
    })
}

pub(super) struct BorrowedReferenceAddress {
    pub(super) descriptor_offset: u32,
    pub(super) referent_offset: u32,
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
