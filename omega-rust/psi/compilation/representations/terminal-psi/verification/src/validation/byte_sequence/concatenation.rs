//! Owned concatenation reads exact live sources and establishes independent
//! backing. Capacity constrains a result; it is never an observed length.
use crate::validation::{
    ModuleError, OperationKind, PlaceId, StructuralAccess, StructuralMultiplicity,
    StructuralPlaceKind, StructuralTypeShape, TerminalMachine, TerminalModule,
    resolve_structural_path,
};
use terminal_psi::control_flow::ByteSequenceOperand;

pub(in crate::validation) fn owned_result(
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
                    OperationKind::EstablishByteSequenceConcatenation { .. }
                )
            {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && result.structural_type == structural_type
                    && result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.projected_qualifications.is_empty()
                    && result.qualification_establishments.is_empty()
                    && result.claims.is_empty()
            })
        })
}

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequenceConcatenation(operation.id);
    let OperationKind::EstablishByteSequenceConcatenation { left, right, .. } = &operation.kind
    else {
        return Err(invalid());
    };
    let result = operation.result.structural().ok_or_else(invalid)?;
    if owned_result(machine, result.place) != Some(result)
        || !matches!(machine.structural_places.iter().find(|row| row.id == result.place).map(|row| row.kind),
            Some(StructuralPlaceKind::OperationResult { producer, .. }) if producer == operation.id)
        || !module.structural_types.iter().any(|row| {
            row.id == result.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        })
    {
        return Err(invalid());
    }
    let required = terminal_semantics::byte_owned_result_predicates(module, machine, result.place)
        .ok_or_else(invalid)?;
    if !required.is_empty()
        && !terminal_semantics::byte_sequence_concatenation_predicates(module, machine, operation)
            .is_some_and(|predicates| predicates.implies(required))
    {
        return Err(invalid());
    }
    for operand in [left, right] {
        if operand.source() == result.place
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == operand.source())
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == operand.source())
        {
            return Err(invalid());
        }
        match operand {
            ByteSequenceOperand::Whole { source, length } => {
                super::length::validate_source(module, machine, operation, *source, invalid)?;
                if !super::read::is_current_length(module, machine, operation, *source, *length) {
                    return Err(invalid());
                }
            }
            ByteSequenceOperand::Field {
                source,
                field,
                length,
            } => {
                let parameter = machine
                    .structural_parameters
                    .iter()
                    .find(|row| row.place == source.place)
                    .ok_or_else(invalid)?;
                // This is a content read, not the metadata-only authority a
                // write-only parameter has for its destination capacity.
                if source.access != StructuralAccess::SharedBorrow
                    || !matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                    || !matches!(
                        parameter.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
                    || !terminal_psi::is_bounded_structural_scalar_store_path(&source.path)
                {
                    return Err(invalid());
                }
                let parent =
                    resolve_structural_path(module, parameter.structural_type, &source.path)
                        .ok_or_else(invalid)?;
                let declaration = module
                    .structural_types
                    .iter()
                    .find(|row| row.id == parent)
                    .ok_or_else(invalid)?;
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return Err(invalid());
                };
                let field = fields
                    .iter()
                    .find(|row| row.id == *field && !row.relevance.is_erased())
                    .ok_or_else(invalid)?;
                if !matches!(field.field_type.byte_sequence_carrier(), Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }))
                    || !crate::validation::structural::byte_sequence_fields::field_content_length_is_current(
                        module, machine, operation.id, source.place, &source.path, field.id, *length,
                    )
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}
