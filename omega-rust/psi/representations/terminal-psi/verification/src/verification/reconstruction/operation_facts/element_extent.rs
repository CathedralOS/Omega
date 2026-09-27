//! Join an element-view length observation to its exact validated producer.

use semantic_vocabulary::{Proposition, StructuralPlaceKind};
use terminal_psi::{
    Operation, OperationKind, StructuralTypeShape, TerminalMachine, TerminalModule,
};
use terminal_semantics::{
    StructuralEffectObservation, element_establishment_length_equation,
    element_subslice_length_equation, structural_effect_leaf_observation,
};

use crate::ModuleError;

/// The structural type reached by a structural argument's path. Each `Field`
/// step resolves inside a `Record`, each `Referent` step crosses a `Reference`
/// carrier — the same projection the establishment validation applies.
fn path_end_type(
    module: &TerminalModule,
    mut current: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    for segment in path {
        let declaration = module
            .structural_types
            .iter()
            .find(|item| item.id == current)?;
        current = match (segment, &declaration.shape) {
            (
                terminal_psi::StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields },
            ) => {
                let field = fields.iter().find(|field| &field.identity == identity)?;
                let terminal_psi::StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                child
            }
            (
                terminal_psi::StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, .. },
            ) => *referent,
            _ => return None,
        };
    }
    Some(current)
}

/// The root structural type a place carries for an element-view source.
fn source_root_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .find(|parameter| parameter.place == place)
    {
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

/// The declared element count of the fixed collection an establishment names.
fn establishment_collection_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<u64> {
    let OperationKind::EstablishElementView { source, .. } = &operation.kind else {
        return None;
    };
    let root = source_root_type(machine, source.place)?;
    let collection = path_end_type(module, root, &source.path)?;
    module
        .structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::FixedArray { length, .. } if declaration.id == collection => {
                Some(length)
            }
            _ => None,
        })
}

/// Relate an `ElementViewLength` result to the extent its exact producer
/// established. Module validation has checked the producer/result join, source
/// type and dominance, so the equation is introduced only at the later read.
/// Signature and block-parameter views carry runtime extents and supply no
/// equation here.
pub(super) fn length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    observation: &StructuralEffectObservation,
) -> Result<Option<Proposition>, ModuleError> {
    let StructuralEffectObservation::ElementViewLengthRead { source, .. } = observation else {
        return Ok(None);
    };
    let Some(place_kind) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *source)
        .map(|place| place.kind)
    else {
        return Ok(None);
    };
    let StructuralPlaceKind::OperationResult { producer, .. } = place_kind else {
        return Ok(None);
    };
    let Some(operation) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)
    else {
        return Ok(None);
    };
    let Some(producer_observation) = structural_effect_leaf_observation(operation)
        .map_err(ModuleError::OperationSemanticSchema)?
    else {
        return Ok(None);
    };
    if let StructuralEffectObservation::ElementViewEstablished { .. } = producer_observation {
        let Some(length) = establishment_collection_length(module, machine, operation) else {
            return Ok(None);
        };
        return element_establishment_length_equation(observation, &producer_observation, length)
            .map_err(ModuleError::OperationSemanticSchema);
    }
    element_subslice_length_equation(observation, &producer_observation)
        .map_err(ModuleError::OperationSemanticSchema)
}
