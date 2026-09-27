//! Independent checks for normalized atomic access events.
//!
//! An `AtomicAccess` operation carries only its root, path, and event. The
//! verifier reconstructs everything the event claims from the module:
//!
//! - the location is one relevant plain scalar field beneath a structural
//!   parameter, reached by the static carrier path a `StructuralScalarFieldStore`
//!   walks — an atomic event joins exactly one location's modification order,
//!   and a runtime-selected element names no single location. A bounded field
//!   is refused: the event carries no range obligation;
//! - the root grants the event's authority: an observing event needs readable
//!   access, a modifying event writable access, and a read-modify-write,
//!   swap, or compare-exchange needs both. A shared borrow therefore admits a
//!   load but no modifying event: Terminal types do not yet mark atomic
//!   cells, and mutation through a shared loan is only sound on such a cell;
//! - the orderings replay the source legality matrix;
//! - the leaf type admits the event: arithmetic fetches need a fixed integer
//!   leaf; loads, stores, swaps, and compare-exchanges take a fixed integer or
//!   Boolean leaf. Address and IEEE leaves have no atomic event;
//! - an observing event defines exactly one scalar result of the leaf type
//!   (the instruction-observed prior) and a store defines none;
//! - every operand is defined earlier and carries the leaf type.
//!
//! The event states no proposition about its result: a serial replay could
//! equate a load with the preceding store, but the concurrent contract lets
//! another participant intervene, so reconstruction treats the result as an
//! unconstrained value of its type and every later primitive snapshot of the
//! leaf as unknown.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{OperationId, PlaceId, ScalarType, StructuralFieldId, ValueId};
use terminal_psi::{
    AtomicAccessEvent, Operation, OperationKind, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};

use super::operations::require_defined;
use super::{IdRegistry, ModuleError, insert_value};

/// Why one atomic access failed verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicAccessRefusal {
    /// The path selects a runtime element, which names no single location.
    RuntimeSelectedLocation,
    /// The orderings violate the source legality matrix.
    IllegalOrdering,
    /// The root does not grant the readable authority an observing event needs.
    UnreadableLocation,
    /// The root does not grant the writable authority a modifying event needs.
    UnwritableLocation,
    /// The carrier path or field names no relevant plain scalar field.
    UnresolvedField,
    /// The leaf type has no event of this kind.
    UnsupportedLeafType(ScalarType),
    /// An observing event without its scalar result, a store with one, or a
    /// result of another type.
    ResultShapeMismatch,
    /// An operand does not carry the leaf type.
    OperandTypeMismatch(ValueId),
}

fn refuse(operation: OperationId, place: PlaceId, refusal: AtomicAccessRefusal) -> ModuleError {
    ModuleError::InvalidAtomicAccess {
        operation,
        place,
        refusal,
    }
}

/// The exact scalar type of the field the event accesses, with the event's
/// authority, location, ordering, and field-type admission rechecked.
pub(super) fn leaf_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: OperationId,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    event: AtomicAccessEvent,
) -> Result<ScalarType, ModuleError> {
    let refuse = |refusal| refuse(operation, place, refusal);
    if !terminal_psi::is_bounded_structural_scalar_store_path(path) {
        return Err(refuse(AtomicAccessRefusal::RuntimeSelectedLocation));
    }
    if !event.ordering_is_legal() {
        return Err(refuse(AtomicAccessRefusal::IllegalOrdering));
    }
    // The root is a structural parameter whose access grants the event:
    // readable to observe, writable to modify, both for an event that does
    // both. Its custody is plain: no claims or qualifications ride on it.
    let parameter = super::structural::scalar_fields::readable_parameter_for(machine, place)
        .ok_or_else(|| refuse(AtomicAccessRefusal::UnreadableLocation))?;
    let readable = matches!(
        parameter.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    );
    let writable = matches!(
        parameter.access,
        StructuralAccess::Owned
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    );
    if event.observes_resident() && !readable {
        return Err(refuse(AtomicAccessRefusal::UnreadableLocation));
    }
    if event.modifies_resident() && !writable {
        return Err(refuse(AtomicAccessRefusal::UnwritableLocation));
    }
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !super::structural::scalar_fields::has_empty_structural_custody(machine, place)
    {
        return Err(refuse(AtomicAccessRefusal::UnreadableLocation));
    }
    let carrier = super::resolve_structural_path(module, parameter.structural_type, path)
        .ok_or_else(|| refuse(AtomicAccessRefusal::UnresolvedField))?;
    let leaf = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields
                .iter()
                .find(|candidate| candidate.id == field && !candidate.relevance.is_erased()),
            _ => None,
        })
        .ok_or_else(|| refuse(AtomicAccessRefusal::UnresolvedField))?;
    // A bounded field owes a range obligation on every store; an atomic event
    // carries none, so only plain scalar fields admit one.
    let StructuralFieldType::Scalar(leaf) = leaf.field_type else {
        return Err(refuse(AtomicAccessRefusal::UnresolvedField));
    };
    let fixed_integer = matches!(leaf, ScalarType::Integer(integer) if !integer.is_address());
    let admitted = match event {
        AtomicAccessEvent::ReadModifyWrite { .. } => fixed_integer,
        AtomicAccessEvent::Load { .. }
        | AtomicAccessEvent::Store { .. }
        | AtomicAccessEvent::Swap { .. }
        | AtomicAccessEvent::CompareExchange { .. } => fixed_integer || leaf == ScalarType::Boolean,
    };
    if !admitted {
        return Err(refuse(AtomicAccessRefusal::UnsupportedLeafType(leaf)));
    }
    Ok(leaf)
}

/// Register the event's result and check its location, authority, orderings
/// and result shape. Operands are checked with the other operand families.
pub(super) fn register(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    registry: &mut IdRegistry,
    value_types: &mut BTreeMap<ValueId, ScalarType>,
) -> Result<(), ModuleError> {
    let OperationKind::AtomicAccess {
        place,
        ref path,
        field,
        event,
    } = operation.kind
    else {
        unreachable!("dispatched an atomic access")
    };
    let leaf = leaf_type(module, machine, operation.id, place, path, field, event)?;
    match (event.observes_resident(), operation.result.scalar()) {
        (true, Some(result)) if result.scalar_type == leaf => {
            insert_value(value_types, &mut registry.values, result.id, leaf)
        }
        (false, None) if operation.result == terminal_psi::OperationResult::Unit => Ok(()),
        _ => Err(refuse(
            operation.id,
            place,
            AtomicAccessRefusal::ResultShapeMismatch,
        )),
    }
}

/// Every operand is defined before the event and carries the leaf type.
pub(super) fn validate_operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::AtomicAccess {
        place,
        ref path,
        field,
        event,
    } = operation.kind
    else {
        unreachable!("dispatched an atomic access")
    };
    let leaf = leaf_type(module, machine, operation.id, place, path, field, event)?;
    for operand in event.operands() {
        require_defined(operand, value_types, defined)?;
        if value_types[&operand] != leaf {
            return Err(refuse(
                operation.id,
                place,
                AtomicAccessRefusal::OperandTypeMismatch(operand),
            ));
        }
    }
    Ok(())
}
