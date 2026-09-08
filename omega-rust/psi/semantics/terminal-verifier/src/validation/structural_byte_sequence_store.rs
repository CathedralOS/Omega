//! Exact non-observing replacement of an inline bounded byte field.

use super::*;

/// Resolve capacity from the destination declaration, never an operation's
/// asserted bound. Proof reconstruction repeats this same semantic join.
pub(crate) fn capacity(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<u64, ModuleError> {
    let invalid = || ModuleError::InvalidStructuralByteSequenceFieldStore(operation.id);
    let OperationKind::StructuralByteSequenceFieldStore {
        destination,
        path,
        field,
        source,
        length,
        ..
    } = &operation.kind
    else {
        return Err(invalid());
    };
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)
        .ok_or_else(invalid)?;
    if operation.result != OperationResult::Unit
        || !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || *destination == *source
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
        || !machine.structural_places.iter().any(|place| {
            place.id == *destination
                && place.kind
                    == StructuralPlaceKind::Parameter {
                        position: parameter.position,
                        is_self: parameter.is_self,
                    }
        })
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == *destination || claim.input == *source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *destination || claim.input.root == *source)
    {
        return Err(invalid());
    }
    super::byte_sequence_length::validate_source(module, machine, operation, *source, invalid)?;
    if !super::byte_sequence_read::is_exact_length(machine, *source, *length) {
        return Err(invalid());
    }
    let parent_type =
        resolve_structural_path(module, parameter.structural_type, path).ok_or_else(invalid)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())
        .ok_or_else(invalid)?;
    let StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
        capacity,
    }) = field.field_type
    else {
        return Err(invalid());
    };
    Ok(capacity)
}
