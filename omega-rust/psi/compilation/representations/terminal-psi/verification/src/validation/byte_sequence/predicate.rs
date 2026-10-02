//! A byte-sequence predicate observes content through the same operand
//! vocabulary a concatenation reads: a whole view carries a direct,
//! dominating length witness, and a field operand reads only under a shared
//! borrow with its field's own current length observation. The evaluation
//! itself is semantics-ordered; this validation binds the read authority.

use crate::validation::{
    ModuleError, ScalarType, StructuralAccess, StructuralFieldId, StructuralTypeShape,
    TerminalMachine, TerminalModule, ValueId, resolve_structural_path,
};
use terminal_psi::control_flow::ByteSequenceOperand;

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    operand: &ByteSequenceOperand,
) -> Result<(), ModuleError> {
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != ScalarType::Boolean)
    {
        return Err(ModuleError::ByteSequencePredicateRequiresBooleanResult(
            operation.id,
        ));
    }
    match operand {
        ByteSequenceOperand::Whole { source, length } => {
            crate::validation::byte_sequence::length::validate_source(
                module,
                machine,
                operation,
                *source,
                || ModuleError::InvalidByteSequencePredicateSource {
                    operation: operation.id,
                    source: *source,
                },
            )?;
            // The length witness must be this operand's own dominating
            // measurement, exactly as an exact read requires.
            if !crate::validation::byte_sequence::read::is_current_length(
                module, machine, operation, *source, *length,
            ) {
                return Err(ModuleError::InvalidByteSequencePredicateLength {
                    operation: operation.id,
                    source: *source,
                    length: *length,
                });
            }
        }
        ByteSequenceOperand::Field {
            source,
            field,
            length,
        } => {
            validate_field(module, machine, operation, source, *field, *length)?;
        }
    }
    Ok(())
}

/// A field operand reads content, not the metadata-only authority a
/// write-only parameter holds for its destination capacity: the argument
/// spells a shared borrow over a bounded path to a byte-sequence field whose
/// own length observation is current at the predicate's site.
fn validate_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: &terminal_psi::StructuralArgument,
    field: StructuralFieldId,
    length: ValueId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidByteSequencePredicateField {
        operation: operation.id,
        source: source.place,
        field,
    };
    let (structural_type, access) =
        terminal_semantics::byte_field_root(machine, source.place).ok_or_else(invalid)?;
    if source.access != StructuralAccess::SharedBorrow
        || !matches!(
            access,
            StructuralAccess::Owned
                | StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
        )
        || !terminal_psi::is_bounded_structural_scalar_store_path(&source.path)
    {
        return Err(invalid());
    }
    let parent =
        resolve_structural_path(module, structural_type, &source.path).ok_or_else(invalid)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|row| row.id == parent)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return Err(invalid());
    };
    let field_row = fields
        .iter()
        .find(|row| row.id == field && !row.relevance.is_erased())
        .ok_or_else(invalid)?;
    if !matches!(
        field_row.field_type.byte_sequence_carrier(),
        Some(
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
        )
    ) || !crate::validation::structural::byte_sequence_fields::field_content_length_is_current(
        module,
        machine,
        operation.id,
        source.place,
        &source.path,
        field_row.id,
        length,
    ) {
        return Err(invalid());
    }
    Ok(())
}
