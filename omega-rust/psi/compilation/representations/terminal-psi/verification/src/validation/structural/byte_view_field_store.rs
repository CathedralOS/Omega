//! Descriptor replacement of one shared borrowed-view field.
//!
//! A stored view must not outlive its backing (wiki/spec/language/lifetimes.md),
//! and Terminal carries no region that could relate an arbitrary view to the
//! destination root. The admitted source is therefore the one whose backing
//! outlives every root: an established byte literal, which denotes immutable
//! image storage rather than any state-local owner. Every other view rejects,
//! even one whose contents equal a literal's. Predicates replay from the
//! literal's exact octets and must imply every declared field predicate.

use crate::validation::{
    ModuleError, OperationKind, OperationResult, StructuralAccess, StructuralPlaceKind,
    StructuralTypeShape, TerminalMachine, TerminalModule,
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
    if !is_literal_source(module, machine, *source) {
        return Err(invalid());
    }
    let declared = field.field_type.byte_predicates();
    if !declared.is_empty()
        && !terminal_semantics::byte_sequence_source_predicates(module, machine, *source)
            .is_some_and(|predicates| predicates.implies(declared))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Whether `source` is the one establishment of a literal place whose
/// declared carrier is a borrowed view. Any other source kind has no lifetime
/// relation to the destination root.
fn is_literal_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: semantic_vocabulary::PlaceId,
) -> bool {
    let Some(StructuralPlaceKind::ByteSequenceLiteral {
        structural_type, ..
    }) = machine
        .structural_places
        .iter()
        .find(|place| place.id == source)
        .map(|place| place.kind)
    else {
        return false;
    };
    module
        .structural_types
        .iter()
        .any(|row| row.id == structural_type && row.shape.is_borrowed_byte_view())
        && machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(&operation.kind,
                    OperationKind::EstablishByteSequenceLiteral { destination, .. }
                        if *destination == source)
            })
            .count()
            == 1
}
