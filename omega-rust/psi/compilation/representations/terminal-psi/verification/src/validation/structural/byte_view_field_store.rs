//! Descriptor replacement of one shared borrowed-view field.
//!
//! The descriptor's exact source is reconstructed at this operation, and all
//! normal exits replay borrowed-output writes. A local backing cannot escape
//! through an out-parameter. Slot-aware loans protect local and caller backing
//! until the stored value's last observation, including copies and successors.
//! Literal image storage uses that same origin judgment. Predicate implication
//! is independent and may require a canonical current-content proof.

use crate::validation::{
    ModuleError, OperationKind, OperationResult, StructuralAccess, StructuralTypeShape,
    TerminalMachine, TerminalModule,
};

/// Validate destination authority, the field carrier, the source's lifetime
/// authority and predicate implication. Availability of the destination home
/// and the source is the ordinary dominance replay, checked separately.
pub(crate) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralByteViewFieldStore(operation.id);
    let OperationKind::StructuralByteViewFieldStore {
        destination,
        path,
        field,
        source,
        predicate_obligation,
    } = &operation.kind
    else {
        return Err(invalid());
    };
    if operation.result != OperationResult::Unit || *destination == *source {
        return Err(invalid());
    }
    let parent_type = super::field_stores::parent_type(module, machine, *destination, path)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .ok_or_else(invalid)?
        .shape
    else {
        return Err(invalid());
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())
        .ok_or_else(invalid)?;
    // Only an immutable shared view field holds a relocatable descriptor:
    // an exclusive view field would duplicate a loan, and a bounded view or
    // owner stores bytes or a bare pointer rather than a descriptor.
    if field.field_type.byte_sequence_carrier()
        != Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        })
    {
        return Err(invalid());
    }
    crate::validation::byte_sequence::length::validate_source(
        module, machine, operation, *source, invalid,
    )?;
    let source_type =
        super::operations::caller_structural_root_type(machine, *source).ok_or_else(invalid)?;
    if !module.structural_types.iter().any(|row| {
        row.id == source_type
            && matches!(
                row.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    }
                )
            )
    }) || terminal_semantics::descriptor_results::stored_source_origins(
        module, machine, operation,
    )
    .is_none()
    {
        return Err(invalid());
    }
    let declared = field.field_type.byte_predicates();
    if predicate_obligation.is_some() {
        if declared.is_empty() || predicate_goal(module, machine, operation).is_none() {
            return Err(invalid());
        }
    } else if !declared.is_empty()
        && !terminal_semantics::byte_sequence_source_predicates(module, machine, *source)
            .is_some_and(|predicates| predicates.implies(declared))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Reconstruct the exact destination declaration and source identity. The
/// obligation is registered and proved separately, before replacement facts.
pub(crate) fn predicate_goal(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<semantic_vocabulary::Proposition> {
    let OperationKind::StructuralByteViewFieldStore {
        destination,
        path,
        field,
        source,
        ..
    } = &operation.kind
    else {
        return None;
    };
    let parent = super::field_stores::parent_type(module, machine, *destination, path)?;
    let StructuralTypeShape::Record { fields } = &module
        .structural_types
        .iter()
        .find(|row| row.id == parent)?
        .shape
    else {
        return None;
    };
    let field = fields
        .iter()
        .find(|row| row.id == *field && !row.relevance.is_erased())?;
    if field.field_type.byte_sequence_carrier()
        != Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        })
    {
        return None;
    }
    let established =
        terminal_semantics::byte_sequence_source_predicates(module, machine, *source)?;
    Some(terminal_semantics::byte_predicate_requirement(
        field.field_type.byte_predicates(),
        established,
        *source,
        &[],
    ))
}
