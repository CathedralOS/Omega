//! Fixed-extent writes through an exact mutable view or established owned result.

use crate::validation::{
    ModuleError, OperationKind, OperationResult, PlaceId, StructuralAccess, StructuralMultiplicity,
    StructuralPlaceKind, StructuralTypeShape, TerminalMachine, TerminalModule,
};
pub(in crate::validation) fn validate_destination(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    destination: PlaceId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequenceWrite(operation.id);
    if super::concatenation::owned_result(machine, destination).is_some() {
        if terminal_semantics::byte_owned_result_predicates(module, machine, destination).is_none()
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == destination)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == destination)
            || !crate::validation::structural::byte_sequence_fields::owned_content_is_current(
                module,
                machine,
                operation.id,
                destination,
            )
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == destination)
        .or_else(|| crate::validation::block_views::parameter(machine, destination))
        .ok_or_else(invalid)?;
    if parameter.access != StructuralAccess::MutableBorrow
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || terminal_semantics::byte_parameter_predicates(module, parameter).is_none()
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
                    }
                    || crate::validation::block_views::parameter(machine, destination)
                        == Some(parameter))
        })
        || !module.structural_types.iter().any(|declaration| {
            declaration.id == parameter.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                            | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                            | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
    {
        return Err(invalid());
    }
    Ok(())
}

pub(in crate::validation) fn validate(
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
    if let Some(predicates) = declared_write_predicates(module, machine, destination) {
        use terminal_psi::ByteSequencePredicate;
        if predicates
            .predicates()
            .any(|predicate| predicate == ByteSequencePredicate::ValidUtf8)
            && !predicates
                .predicates()
                .any(|predicate| predicate == ByteSequencePredicate::AsciiOnly)
            && !crate::validation::structural::byte_sequence_fields::owned_content_is_ascii(
                module,
                machine,
                operation.id,
                destination,
            )
        {
            return Err(ModuleError::InvalidByteSequenceWrite(operation.id));
        }
    }
    let producer = machine.blocks.iter().flat_map(|block| &block.operations)
        .find(|candidate| candidate.result.scalar().is_some_and(|result| result.id == length)
            && matches!(candidate.kind, OperationKind::ByteSequenceLength { source } if source == destination))
        .ok_or(ModuleError::InvalidByteSequenceWrite(operation.id))?;
    // A block binding can be re-established on a later visit. Its witness
    // must be read in this same block execution, not a prior binding.
    if crate::validation::block_views::parameter(machine, destination).is_some()
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
        || !crate::validation::structural::byte_sequence_fields::view_length_is_current(
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

/// A store preserves the destination's declared predicates regardless of whether
/// this machine owns the backing or borrows it from its caller. Incidental
/// initializer facts are not invariants and must not enter this goal.
fn declared_write_predicates(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: PlaceId,
) -> Option<terminal_psi::ByteSequencePredicateSet> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == destination)
        .or_else(|| crate::validation::block_views::parameter(machine, destination))
    {
        terminal_semantics::byte_parameter_predicates(module, parameter)
    } else {
        terminal_semantics::byte_owned_result_predicates(module, machine, destination)
    }
}

/// Every byte write preserves its destination's declared primitive predicates.
pub(crate) fn byte_store_goal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    bound: semantic_vocabulary::Proposition,
) -> Result<semantic_vocabulary::Proposition, ModuleError> {
    let OperationKind::ByteSequenceWrite {
        destination, value, ..
    } = operation.kind
    else {
        return Ok(bound);
    };
    validate_destination(module, machine, operation, destination)?;
    let predicates = declared_write_predicates(module, machine, destination)
        .ok_or(ModuleError::InvalidByteSequenceWrite(operation.id))?;
    terminal_semantics::indexed_byte_store_goal(bound, value, predicates).map_err(|error| {
        ModuleError::OperationSemanticSchema(
            terminal_semantics::OperationSemanticError::InvalidProposition(error),
        )
    })
}
