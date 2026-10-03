//! Exact immutable borrowed windows; establishment is checked at every use.

use crate::validation::{
    BTreeSet, ModuleError, OperationKind, PlaceId, StructuralAccess, StructuralPlaceKind,
    StructuralTypeShape, TerminalMachine, TerminalModule, ValueId,
};
/// Authenticate view result custody against its current module and producer.
pub(in crate::validation) fn borrowed_result_in_module<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    crate::validation::element_view::view_loan_result_in_module(module, machine, place)
        .map(|(result, _)| result)
}

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequenceSubslice(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    if borrowed_result_in_module(module, machine, result.place) != Some(result)
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
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                )
        })
    {
        return Err(invalid());
    }
    crate::validation::byte_sequence::length::validate_source(
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
        || !crate::validation::byte_sequence::read::is_exact_length(machine, source, length)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Block-local views and exact borrowed results need dominance. Other structural
/// operations retain their existing ownership/frontier admission rules.
pub(in crate::validation) fn validate_uses(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let require = |place| {
        if (borrowed_result_in_module(module, machine, place).is_some()
            || super::concatenation::owned_result(machine, place).is_some()
            || terminal_semantics::byte_owned_call_result(module, machine, place).is_some()
            || machine.structural_parameters.iter().any(|parameter| {
                parameter.place == place
                    && parameter.access == StructuralAccess::MutableBorrow
                    && module.structural_types.iter().any(|declaration| {
                        declaration.id == parameter.structural_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                                        | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            )
                    })
            })
            || crate::validation::block_views::parameter(machine, place).is_some()
            || machine.structural_places.iter().any(|declaration| {
                declaration.id == place
                    && matches!(
                        declaration.kind,
                        StructuralPlaceKind::ByteSequenceLiteral { .. }
                    )
            }))
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
    // Field-root declaration custody does not establish an operation result or
    // a state-local name. Those homes must dominate this exact observation.
    let require_field = |place| {
        if machine.structural_places.iter().any(|declaration| {
            declaration.id == place
                && matches!(
                    declaration.kind,
                    StructuralPlaceKind::OperationResult { .. }
                        | StructuralPlaceKind::BlockParameter { .. }
                )
        }) && !available.contains(&place)
        {
            return Err(ModuleError::ByteSequenceViewNotEstablished {
                operation: operation.id,
                place,
            });
        }
        require(place)
    };
    let require_operand = |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand
    {
        terminal_psi::control_flow::ByteSequenceOperand::Field { source, .. } => {
            require_field(source.place)
        }
        _ => require(operand.source()),
    };
    match &operation.kind {
        OperationKind::EstablishByteSequenceConcatenation { left, right, .. } => {
            require_operand(left)?;
            require_operand(right)
        }
        OperationKind::StructuralByteSequenceFieldLength { source, .. }
        | OperationKind::StructuralByteSequenceFieldRead { source, .. } => require_field(*source),
        OperationKind::ByteSequenceStore {
            destination,
            source,
            ..
        } => {
            // A state edge moves exclusive use of a whole owner to its new
            // block name. A valid payload does not revive an old destination.
            require(*destination)?;
            require(*source)
        }
        // The replaced field's home and the stored view must both be live
        // here: a later arrival's establishment supplies neither.
        OperationKind::StructuralByteViewFieldStore {
            destination,
            source,
            ..
        } => {
            require_field(*destination)?;
            require(*source)
        }
        OperationKind::ByteSequenceLength { source }
        | OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. }
        | OperationKind::ByteSequenceRead { source, .. }
        | OperationKind::StructuralByteSequenceFieldStore { source, .. }
        | OperationKind::ByteSequenceSubslice { source, .. } => require(*source),
        OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::StructuralScalarFieldStore { destination, .. } => require(*destination),
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
