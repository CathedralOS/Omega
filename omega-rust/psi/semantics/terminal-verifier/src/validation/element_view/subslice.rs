//! Exact immutable element-view subslices; establishment is checked at every use.

use crate::validation::{
    BTreeSet, ModuleError, OperationKind, PlaceId, StructuralMultiplicity, StructuralPlaceKind,
    StructuralTypeShape, TerminalMachine, TerminalModule, ValueId,
};

pub(in crate::validation) fn borrowed_result(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if operation.id != producer
                || !matches!(
                    operation.kind,
                    OperationKind::EstablishElementView { .. }
                        | OperationKind::ElementViewSubslice { .. }
                )
            {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && result.structural_type == structural_type
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
        })
}

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidElementViewSubslice(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    if borrowed_result(machine, result.place) != Some(result)
        || !matches!(machine.structural_places.iter().find(|row| row.id == result.place).map(|row| row.kind),
            Some(StructuralPlaceKind::OperationResult { producer, .. }) if producer == operation.id)
        || result.place == source
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == source || claim.input == result.place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source || claim.input.root == result.place)
        || !module.structural_types.iter().any(|row| {
            row.id == result.structural_type
                && matches!(row.shape, StructuralTypeShape::ElementView { .. })
        })
    {
        return Err(invalid());
    }
    crate::validation::element_view::length::validate_source(
        module, machine, operation, source, invalid,
    )?;
    let source_type = machine
        .structural_parameters
        .iter()
        .find(|row| row.place == source)
        .map(|row| row.structural_type)
        .or_else(|| {
            crate::validation::block_views::parameter(machine, source)
                .map(|row| row.structural_type)
        })
        .or_else(|| {
            machine
                .structural_places
                .iter()
                .find_map(|row| match row.kind {
                    StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } if row.id == source => Some(structural_type),
                    _ => None,
                })
        });
    if source_type != Some(result.structural_type)
        || !crate::validation::element_view::length::is_exact_length(machine, source, length)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Exact borrowed results need dominance: a view minted by an operation is
/// observable only where that operation's block dominates. A machine-level
/// shared view parameter is the machine's own input, established at entry,
/// which dominates every block, so it needs no definition site; mutable views
/// keep their separate per-block availability. Other structural operations
/// retain their existing ownership/frontier admission rules.
pub(in crate::validation) fn validate_uses(
    _module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let require = |place: PlaceId| {
        if borrowed_result(machine, place).is_some() && !available.contains(&place) {
            Err(ModuleError::ElementViewNotEstablished {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    match &operation.kind {
        OperationKind::ElementViewLength { source }
        | OperationKind::ElementViewRead { source, .. }
        | OperationKind::ElementViewSubslice { source, .. } => require(*source),
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                require(argument.place)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
