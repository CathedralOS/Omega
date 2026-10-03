//! Immutable element-view establishment over a contiguous structural collection.

use crate::validation::{
    BTreeSet, IntegerSign, IntegerType, ModuleError, OperationKind, PlaceId, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceKind,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// Resolve the collection and its access together. Owning a descriptor's
/// storage never grants access beyond the reference it contains: every
/// dereference must itself cover the requested loan, including intermediate
/// references beneath fields or array elements.
pub(in crate::validation) fn source_collection_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &terminal_psi::StructuralArgument,
) -> Option<StructuralTypeId> {
    let (mut current, access) = source_root(machine, source.place)?;
    let can_supply = super::super::structural::operations::structural_access_can_supply;
    if !can_supply(access, source.access) {
        return None;
    }
    for segment in &source.path {
        let declaration = module
            .structural_types
            .iter()
            .find(|item| item.id == current)?;
        current = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_), _) => {
                terminal_semantics::canonical_static_projection(
                    module.structural_types.iter(),
                    current,
                    std::slice::from_ref(segment),
                )?
                .1
                .id
            }
            (
                StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, access },
            ) if can_supply(*access, source.access) => *referent,
            // A byte window keeps its fixed-array backing's element type: the
            // view minted here reads `u8` elements out of the container the
            // path names, and the segment's own contract bounds it.
            (
                StructuralPathSegment::FixedByteRange { start, end },
                StructuralTypeShape::FixedArray { length, .. },
            ) if start <= end && end <= length => current,
            _ => return None,
        };
    }
    Some(current)
}

/// Rejoin the source's element extent without treating a pointer carrier as
/// inline bytes. Borrowed bounded byte backing produces a byte descriptor;
/// it cannot reinterpret an existing byte carrier as an element view.
pub(in crate::validation) fn collection_source_matches(
    module: &TerminalModule,
    collection: StructuralTypeId,
    element: StructuralTypeId,
    view_type: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
    access: StructuralAccess,
) -> bool {
    match terminal_semantics::element_view_region_geometry(
        module.structural_types.iter(),
        collection,
        element,
        region,
        access,
    ) {
        Some(terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing)) => {
            backing.storage == terminal_semantics::CollectionStorage::Inline
                || module
                    .structural_types
                    .iter()
                    .any(|row| row.id == view_type && row.shape.is_borrowed_byte_view())
        }
        Some(terminal_semantics::ElementViewRegionGeometry::RepresentationBytes { .. }) => {
            collection_view_result_matches(module, view_type, element)
        }
        None => false,
    }
}

/// Arrival custody comes from the exact declaration. A structural result owns
/// its produced value's storage; its producer and type must rejoin the place
/// declaration. Reference permission remains a separate path-step judgment.
fn source_root(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<(StructuralTypeId, StructuralAccess)> {
    if let Some(parameter) = terminal_semantics::arrival_parameter(machine, place) {
        return Some((parameter.structural_type, parameter.access));
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = machine
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?
        .kind
    else {
        return None;
    };
    let result = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?
        .result
        .structural()?;
    (result.place == place && result.structural_type == structural_type)
        .then_some((structural_type, StructuralAccess::Owned))
}

/// Both readable loan modes establish the same collection descriptor. Byte
/// descriptors omit an element field, so their borrowed carrier must rejoin
/// the exact u8 element declaration. This never converts an existing view
/// descriptor into another carrier or grants additional source access.
pub(in crate::validation) fn collection_view_result_matches(
    module: &TerminalModule,
    view_type: StructuralTypeId,
    element: StructuralTypeId,
) -> bool {
    module.structural_types.iter().any(|declaration| {
        declaration.id == view_type
            && match &declaration.shape {
                StructuralTypeShape::ElementView {
                    element: result_element,
                } => *result_element == element,
                // A `&[u8]` byte view is this same establishment over `u8`
                // elements; its carrier keeps no element field, so the op's
                // minted element must itself resolve to the `u8` scalar row.
                StructuralTypeShape::ByteSequence(_) => {
                    declaration.shape.is_borrowed_byte_view()
                        && module.structural_types.iter().any(|element_declaration| {
                            element_declaration.id == element
                                && matches!(
                                    element_declaration.shape,
                                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                                        ref integer
                                    )) if *integer
                                        == IntegerType::new(IntegerSign::Unsigned, 8)
                                            .expect("u8 is valid")
                                )
                        })
                }
                _ => false,
            }
    })
}

pub(in crate::validation) fn validate_establishment(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    destination: PlaceId,
    source: &terminal_psi::StructuralArgument,
    element: StructuralTypeId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidElementViewEstablishment(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    if result.place != destination || result.place == source.place {
        return Err(invalid());
    }
    if !matches!(
        machine
            .structural_places
            .iter()
            .find(|row| row.id == result.place)
            .map(|row| row.kind),
        Some(StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        }) if producer == operation.id && structural_type == result.structural_type
    ) {
        return Err(invalid());
    }
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid());
    }
    if !collection_view_result_matches(module, result.structural_type, element) {
        return Err(invalid());
    }
    if source.access != StructuralAccess::SharedBorrow {
        return Err(invalid());
    }
    let collection = source_collection_type(module, machine, source).ok_or_else(invalid)?;
    let OperationKind::EstablishElementView { region, .. } = operation.kind else {
        return Err(invalid());
    };
    if matches!(
        region,
        terminal_psi::ElementViewRegion::RepresentationBytes { .. }
    ) && source
        .path
        .iter()
        .any(|segment| matches!(segment, StructuralPathSegment::FixedByteRange { .. }))
    {
        return Err(invalid());
    }
    if !collection_source_matches(
        module,
        collection,
        element,
        result.structural_type,
        region,
        source.access,
    ) {
        return Err(invalid());
    }
    Ok(())
}

/// The view's collection root must exist on every arrival; projections into it
/// are resolved by `source_collection_type`.
pub(in crate::validation) fn validate_uses(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    if let OperationKind::EstablishElementView { source, .. } = &operation.kind {
        let parameter = machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == source.place);
        if !parameter && !available.contains(&source.place) {
            return Err(ModuleError::ElementViewNotEstablished {
                operation: operation.id,
                place: source.place,
            });
        }
    }
    Ok(())
}
