//! Immutable element-view establishment over a contiguous structural collection.

use crate::validation::{
    BTreeSet, ModuleError, OperationKind, PlaceId, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// The structural type reached by a structural argument's path: each `Field`
/// step resolves inside a `Record`, each `Referent` step crosses a `Reference`
/// carrier. Returns the collection type the path names.
fn path_end_type(
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
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields.iter().find(|field| &field.identity == identity)?;
                if field.relevance != terminal_psi::BindingRelevance::Relevant {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                child
            }
            (StructuralPathSegment::Referent, StructuralTypeShape::Reference { referent, .. }) => {
                *referent
            }
            _ => return None,
        };
    }
    Some(current)
}

/// The element type of the contiguous collection the source path names.
fn collection_element(
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

/// The root structural type a place carries when an element view may borrow it.
fn source_root_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = crate::validation::block_views::parameter(machine, place) {
        return Some(parameter.structural_type);
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
            && matches!(
                declaration.shape,
                StructuralTypeShape::ElementView {
                    element: result_element,
                } if result_element == element
            )
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
