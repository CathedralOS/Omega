//! Whole bounded-owner replacement preserves its declared byte predicates.
use crate::validation::{
    ModuleError, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(crate) fn capacity(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<u64, ModuleError> {
    let invalid = || ModuleError::InvalidStructuralByteSequenceFieldStore(operation.id);
    let OperationKind::ByteSequenceStore {
        destination,
        source,
        length,
        ..
    } = operation.kind
    else {
        return Err(invalid());
    };
    let parameter =
        terminal_semantics::arrival_parameter(machine, destination).ok_or_else(invalid)?;
    if operation.result != OperationResult::Unit
        || destination == source
        || !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == destination || claim.input == source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == destination || claim.input.root == source)
    {
        return Err(invalid());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|row| row.id == parameter.structural_type)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    }) = declaration.shape
    else {
        return Err(invalid());
    };
    let required =
        terminal_semantics::byte_parameter_predicates(module, parameter).ok_or_else(invalid)?;
    super::length::validate_replacement_source(module, machine, operation, source, invalid)?;
    if !super::read::is_current_length(module, machine, operation, source, length)
        || (!required.is_empty()
            && !terminal_semantics::byte_sequence_source_predicates(module, machine, source)
                .is_some_and(|predicates| predicates.implies(required)))
    {
        return Err(invalid());
    }
    Ok(capacity)
}
