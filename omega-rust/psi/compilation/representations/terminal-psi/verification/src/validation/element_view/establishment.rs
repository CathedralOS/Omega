//! Immutable element-view establishment over a contiguous structural collection.

use crate::validation::{
    BTreeSet, IntegerSign, IntegerType, ModuleError, OperationKind, PlaceId, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceKind,
    StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// Resolve ordinary storage projections through the shared type walker.
/// Reference crossings and byte windows retain their distinct semantics;
/// neither is an implicit field/index projection.
pub(in crate::validation) fn path_end_type(
    module: &TerminalModule,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
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
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
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

/// The element type of the contiguous collection the source path names.
pub(in crate::validation) fn collection_element(
    module: &TerminalModule,
    collection: StructuralTypeId,
) -> Option<StructuralTypeId> {
    module
        .structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::FixedArray { element, .. } if declaration.id == collection => {
                Some(element)
            }
            _ => None,
        })
}

/// The readable root type a collection view may borrow. An argument's access
/// cannot grant authority absent from its backing: both shared and mutable
/// views expose reads, so write-only parameters cannot establish either one.
pub(in crate::validation) fn source_root_type(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return (parameter.access != StructuralAccess::WriteOnlyBorrow)
            .then_some(parameter.structural_type);
    }
    if let Some(parameter) = crate::validation::block_views::parameter(machine, place) {
        return (parameter.access != StructuralAccess::WriteOnlyBorrow)
            .then_some(parameter.structural_type);
    }
    machine
        .structural_places
        .iter()
        .find_map(|row| match row.kind {
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            } if row.id == place => Some(structural_type),
            _ => None,
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
    if !module.structural_types.iter().any(|declaration| {
        declaration.id == result.structural_type
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
    }) {
        return Err(invalid());
    }
    if source.access != StructuralAccess::SharedBorrow {
        return Err(invalid());
    }
    let root = source_root_type(machine, source.place).ok_or_else(invalid)?;
    let collection = path_end_type(module, root, &source.path).ok_or_else(invalid)?;
    if collection_element(module, collection) != Some(element) {
        return Err(invalid());
    }
    Ok(())
}

/// The view's collection root must exist on every arrival; projections into it
/// are resolved by `path_end_type`.
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
