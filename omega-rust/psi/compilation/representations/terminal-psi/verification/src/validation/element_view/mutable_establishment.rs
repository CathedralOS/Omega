//! Mutable element-view establishment over a contiguous structural collection.

use crate::validation::element_view::establishment::{collection_element, path_end_type};
use crate::validation::{
    BTreeSet, ModuleError, OperationKind, PlaceId, StructuralAccess, StructuralMultiplicity,
    StructuralPlaceKind, StructuralTypeId, StructuralTypeShape, TerminalMachine, TerminalModule,
};

/// A mutable establishment mirrors the immutable sibling's result shape and
/// source resolution; the custody differences are the argument access —
/// `MutableBorrow` marks the exclusive loan the checked planner rosters — and
/// the result multiplicity, which spells `Affine`: the loan's holder dies with
/// its binding's scope.
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
    if result.multiplicity != StructuralMultiplicity::Affine
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
    if source.access != StructuralAccess::MutableBorrow {
        return Err(invalid());
    }
    let root = super::establishment::source_root_type(machine, source.place).ok_or_else(invalid)?;
    let collection = path_end_type(module, root, &source.path).ok_or_else(invalid)?;
    if collection_element(module, collection) != Some(element) {
        return Err(invalid());
    }
    Ok(())
}

/// The mutable view's collection root must exist on every arrival, exactly
/// like the shared establishment's: a mutable loan does not widen what a
/// block may project.
pub(in crate::validation) fn validate_uses(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    if let OperationKind::EstablishMutableElementView { source, .. } = &operation.kind {
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
