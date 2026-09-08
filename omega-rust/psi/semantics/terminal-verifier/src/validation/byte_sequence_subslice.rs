//! Exact immutable borrowed windows; establishment is checked at every use.

use super::*;

pub(super) fn borrowed_result(
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
                || !matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
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

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequenceSubslice(operation.id);
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
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
        })
    {
        return Err(invalid());
    }
    super::byte_sequence_length::validate_source(module, machine, operation, source, invalid)?;
    let source_type = machine
        .structural_parameters
        .iter()
        .find(|row| row.place == source)
        .map(|row| row.structural_type)
        .or_else(|| super::block_views::parameter(machine, source).map(|row| row.structural_type))
        .or_else(|| {
            machine
                .structural_places
                .iter()
                .find_map(|row| match row.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        structural_type, ..
                    }
                    | StructuralPlaceKind::OperationResult {
                        structural_type, ..
                    } if row.id == source => Some(structural_type),
                    _ => None,
                })
        });
    if source_type != Some(result.structural_type)
        || !super::byte_sequence_read::is_exact_length(machine, source, length)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Block-local views and exact borrowed results need dominance. Other structural
/// operations retain their existing ownership/frontier admission rules.
pub(super) fn validate_uses(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let require = |place| {
        if (borrowed_result(machine, place).is_some()
            || super::block_views::parameter(machine, place).is_some())
            && !available.contains(&place)
        {
            Err(ModuleError::ByteSequenceViewNotEstablished {
                operation: operation.id,
                place,
            })
        } else {
            Ok(())
        }
    };
    match &operation.kind {
        OperationKind::ByteSequenceLength { source }
        | OperationKind::ByteSequenceRead { source, .. }
        | OperationKind::ByteSequenceSubslice { source, .. } => require(*source),
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
        } => {
            for argument in structural_arguments {
                require(argument.place)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
