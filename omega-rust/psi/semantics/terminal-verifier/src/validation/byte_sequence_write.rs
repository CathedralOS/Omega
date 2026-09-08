//! Fixed-extent writes through an exact unrestricted mutable view parameter.
use super::*;

pub(super) fn validate_destination(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    destination: PlaceId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequenceWrite(operation.id);
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == destination)
        .or_else(|| super::block_views::parameter(machine, destination))
        .ok_or_else(invalid)?;
    if parameter.access != StructuralAccess::MutableBorrow
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == destination)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == destination)
        || !machine.structural_places.iter().any(|place| {
            place.id == destination
                && (place.kind
                    == StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    }
                    || super::block_views::parameter(machine, destination) == Some(parameter))
        })
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && declaration.shape
                    == StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView,
                    )
        })
    {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let OperationKind::ByteSequenceWrite {
        destination,
        length,
        ..
    } = operation.kind
    else {
        return Err(ModuleError::InvalidByteSequenceWrite(operation.id));
    };
    validate_destination(module, machine, operation, destination)?;
    let producer = machine.blocks.iter().flat_map(|block| &block.operations)
        .find(|candidate| candidate.result.scalar().is_some_and(|result| result.id == length)
            && matches!(candidate.kind, OperationKind::ByteSequenceLength { source } if source == destination))
        .ok_or(ModuleError::InvalidByteSequenceWrite(operation.id))?;
    // A block binding can be re-established on a later visit. Its witness
    // must be read in this same block execution, not a prior binding.
    if super::block_views::parameter(machine, destination).is_some()
        && !machine.blocks.iter().any(|block| {
            block
                .operations
                .iter()
                .any(|candidate| candidate.id == producer.id)
                && block
                    .operations
                    .iter()
                    .any(|candidate| candidate.id == operation.id)
        })
    {
        return Err(ModuleError::InvalidByteSequenceWrite(operation.id));
    }
    if operation.result != OperationResult::Unit
        || !super::structural_byte_sequence_fields::view_length_is_current(
            module,
            machine,
            producer.id,
            operation.id,
            destination,
        )
    {
        return Err(ModuleError::InvalidByteSequenceWrite(operation.id));
    }
    Ok(())
}
