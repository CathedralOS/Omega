//! Exact non-observing replacement of an inline bounded byte field.

use crate::validation::{
    ModuleError, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachine, TerminalModule, resolve_structural_path,
};
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
    let reference = crate::validation::references::mutable_record_field_destination(
        module,
        machine,
        *destination,
        path,
    );
    let parameter = terminal_semantics::arrival_parameter(machine, *destination);
    let (root_type, carrier_path) = if let Some(reference) = reference {
        reference
    } else if let Some(result) =
        crate::validation::record::completed_source(module, machine, *destination).filter(
            |result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && result.qualification_establishments.is_empty()
                    && module.structural_types.iter().any(|declaration| {
                        declaration.id == result.structural_type
                            && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                    })
            },
        )
    {
        // The result's establishment and dominance are independently checked;
        // replacement preserves this owned home's exact declared field type.
        (result.structural_type, path.as_slice())
    } else {
        let parameter = parameter.ok_or_else(invalid)?;
        if !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(invalid());
        }
        (parameter.structural_type, path.as_slice())
    };
    if operation.result != OperationResult::Unit
        || *destination == *source
        || !terminal_psi::is_bounded_structural_scalar_store_path(carrier_path)
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
    crate::validation::byte_sequence::length::validate_replacement_source(
        module, machine, operation, *source, invalid,
    )?;
    if !crate::validation::byte_sequence::read::is_current_length(
        module, machine, operation, *source, *length,
    ) {
        return Err(invalid());
    }
    let parent_type =
        resolve_structural_path(module, root_type, carrier_path).ok_or_else(invalid)?;
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
    let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) =
        field.field_type.byte_sequence_carrier()
    else {
        return Err(invalid());
    };
    if !field.field_type.byte_predicates().is_empty()
        && !terminal_semantics::byte_sequence_source_predicates(module, machine, *source)
            .is_some_and(|predicates| predicates.implies(field.field_type.byte_predicates()))
    {
        return Err(invalid());
    }
    Ok(capacity)
}
