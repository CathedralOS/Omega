//! Initialized, claim-free primitive storage and its exact access permissions.

use super::*;

pub(super) fn local_result(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|entry| entry.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    if !matches!(
        operation.kind,
        OperationKind::EstablishPrimitiveLocal { .. }
    ) {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place))
    .then_some(result)
}

pub(super) fn scalar_type(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
) -> Option<ScalarType> {
    module.structural_types.iter().find_map(|declaration| {
        if declaration.id != structural_type {
            return None;
        }
        match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => Some(scalar_type),
            _ => None,
        }
    })
}

pub(super) fn validate_establishment(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<ScalarType, ModuleError> {
    let invalid = || ModuleError::InvalidPrimitiveLocalEstablishment(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    if local_result(machine, result.place) != Some(result)
        || !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && matches!(place.kind, StructuralPlaceKind::OperationResult { producer, .. }
                if producer == operation.id)
        })
    {
        return Err(invalid());
    }
    scalar_type(module, result.structural_type).ok_or_else(invalid)
}

fn parameter_type(
    machine: &TerminalMachine,
    place: PlaceId,
    writing: bool,
) -> Option<StructuralTypeId> {
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|entry| entry.place == place)?;
    let allowed = if writing {
        matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
    } else {
        matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
    };
    (allowed
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place)
        && machine.structural_places.iter().any(|entry| {
            entry.id == place
                && matches!(entry.kind, StructuralPlaceKind::Parameter { position, is_self }
                if position == parameter.position && is_self == parameter.is_self)
        }))
    .then_some(parameter.structural_type)
}

pub(super) fn store_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    place: PlaceId,
) -> Result<ScalarType, ModuleError> {
    let structural_type = parameter_type(machine, place, true)
        .or_else(|| local_result(machine, place).map(|result| result.structural_type))
        .ok_or(ModuleError::WriteOnlyPrimitiveStoreDestinationMismatch { operation, place })?;
    scalar_type(module, structural_type).ok_or(
        ModuleError::WriteOnlyPrimitiveStoreRequiresPrimitiveScalar {
            operation,
            structural_type,
        },
    )
}

pub(super) fn read_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    place: PlaceId,
) -> Result<ScalarType, ModuleError> {
    let invalid = || ModuleError::InvalidPrimitiveScalarRead { operation, place };
    let structural_type = parameter_type(machine, place, false)
        .or_else(|| local_result(machine, place).map(|result| result.structural_type))
        .ok_or_else(invalid)?;
    scalar_type(module, structural_type).ok_or_else(invalid)
}

pub(super) fn validate_uses(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let require_available = |place| {
        if local_result(machine, place).is_some() && !available.contains(&place) {
            Err(ModuleError::PrimitiveLocalNotAvailable {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    match &operation.kind {
        OperationKind::PrimitiveScalarRead { source } => require_available(*source)?,
        OperationKind::WriteOnlyPrimitiveStore { destination, .. } => {
            require_available(*destination)?
        }
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require_available(argument.place)?;
            }
        }
        _ => {}
    }
    Ok(())
}
