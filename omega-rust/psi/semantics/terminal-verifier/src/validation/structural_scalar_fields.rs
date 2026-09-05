//! Bounded scalar-field mutation and observation below structural parameters.

use super::*;
use terminal_psi::is_bounded_structural_scalar_store_path;

fn parameter_for(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)?;
    machine
        .structural_places
        .iter()
        .any(|declaration| {
            declaration.id == place
                && matches!(
                    declaration.kind,
                    StructuralPlaceKind::Parameter { position, is_self }
                        if position == parameter.position && is_self == parameter.is_self
                )
        })
        .then_some(parameter)
}

fn direct_relevant_scalar_field(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    fields.iter().find_map(|candidate| {
        (candidate.id == field && !candidate.relevance.is_erased())
            .then_some(&candidate.field_type)
            .and_then(|field_type| match field_type {
                StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
                StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::Structural(_)
                | StructuralFieldType::Erased { .. } => None,
            })
    })
}

fn has_empty_structural_custody(machine: &TerminalMachine, place: PlaceId) -> bool {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .is_some_and(|parameter| {
            parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty()
        })
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place)
}

fn has_readable_structural_access(access: StructuralAccess) -> bool {
    matches!(
        access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    )
}

pub(super) fn structural_scalar_field_store_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    destination: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Result<ScalarType, ModuleError> {
    let invalid = || ModuleError::InvalidStructuralScalarFieldStore {
        operation,
        destination,
        path: path.to_vec(),
        field,
    };
    let parameter = parameter_for(machine, destination).ok_or_else(invalid)?;
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || !has_empty_structural_custody(machine, destination)
        || !is_bounded_structural_scalar_store_path(path)
    {
        return Err(invalid());
    }
    let parent_type =
        resolve_structural_path(module, parameter.structural_type, path).ok_or_else(invalid)?;
    direct_relevant_scalar_field(module, parent_type, field).ok_or_else(invalid)
}

pub(super) fn validate_integer_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    field: StructuralFieldId,
    result_type: ScalarType,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidIntegerStructuralField {
        operation,
        source,
        field,
    };
    let parameter = parameter_for(machine, source).ok_or_else(invalid)?;
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !has_readable_structural_access(parameter.access)
        || !has_empty_structural_custody(machine, source)
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|candidate| {
                matches!(
                    candidate.kind,
                    OperationKind::IntegerStructuralField {
                        source: other_source,
                        field: other_field,
                    } if (other_source, other_field) != (source, field)
                )
            })
    {
        return Err(invalid());
    }
    let ScalarType::Integer(_) = result_type else {
        return Err(ModuleError::IntegerStructuralFieldRequiresIntegerResult(
            operation,
        ));
    };
    if direct_relevant_scalar_field(module, parameter.structural_type, field) != Some(result_type) {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate_boolean_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    source: PlaceId,
    field: StructuralFieldId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidBooleanStructuralField {
        operation,
        source,
        field,
    };
    let parameter = parameter_for(machine, source).ok_or_else(invalid)?;
    if parameter.access == StructuralAccess::WriteOnlyBorrow {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess { operation, source });
    }
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !has_readable_structural_access(parameter.access)
        || !has_empty_structural_custody(machine, source)
        || machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|candidate| {
                matches!(
                    candidate.kind,
                    OperationKind::BooleanStructuralField {
                        source: other_source,
                        field: other_field,
                    } if (other_source, other_field) != (source, field)
                )
            })
        || direct_relevant_scalar_field(module, parameter.structural_type, field)
            != Some(ScalarType::Boolean)
    {
        return Err(invalid());
    }
    Ok(())
}
