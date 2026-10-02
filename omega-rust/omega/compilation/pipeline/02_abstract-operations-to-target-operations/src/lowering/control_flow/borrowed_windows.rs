//! Borrowed-window field replacement realized as two byte copies.
//!
//! `MoveStructuralField` vacates one declared field beneath a mutable-borrowed
//! parameter into a fresh owned result; `StoreStructuralField` reseats exactly
//! that vacancy with an owned subtree of the declared type. The Terminal
//! verifier proved the window discipline before these rows reach Omega: the
//! move opens restoration debt on the root, no operation, edge or case
//! dispatch observes the absent subtree, and the store closes the debt before
//! every non-crash exit (terminal-psi-to-abstract-operations
//! `machine/operation/borrowed_windows.rs`).
//!
//! At the machine level the move is the byte copy a structural leaf copy
//! already performs: the field's bytes at their resolved offset beneath the
//! root's referent into the result's fresh home. It therefore lowers to
//! `TargetUnitOperation::StructuralLeafCopy` over the spelled path extended by
//! the field's own segment and shares that row's legalization and selection.
//! The vacated bytes stay in place; the window forbids observing them, so
//! clearing them would be dead work. The store is the inverse copy, the value
//! home's bytes back into the field, retained as
//! `TargetUnitOperation::StoreStructuralField`; selection realizes both
//! directions with one chunked copy routine rather than a second copy machine.
//!
//! Only plain subtrees move this way. A reference-bearing field carries loan
//! custody a byte copy cannot relocate, so it keeps rejecting here.
//!
//! A root window (`field: None`, empty path) is whole-value assignment
//! through an exclusive loan: both copies span the whole referent at offset
//! zero beneath the loan's pointer, and only a borrowed-parameter root
//! qualifies.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::StructuralFieldId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathSegment};

pub(super) fn move_field(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::MoveStructuralField {
        psi_operation,
        result,
        source,
        path,
        field,
    } = operation
    else {
        return Err(invalid());
    };
    let referent = window_referent(function, live, types, source, path)?;
    if !window_root(function, prepared, live, source)
        || !root_window_loan(source, path, *field)
        || super::references::contains_reference(types, result.structural_type)
    {
        return Err(invalid());
    }
    let (geometry_type, geometry_path) = referent
        .as_ref()
        .map_or((source.structural_type(), path.as_slice()), |referent| {
            (referent.referent, referent.path.as_slice())
        });
    let mut path = field_path(types, geometry_type, geometry_path, *field).ok_or_else(invalid)?;
    if referent.is_some() {
        path.insert(0, StructuralPathSegment::Referent);
    }
    super::leaf_copy::copy_extent(
        *psi_operation,
        result,
        source.place(),
        &path.into(),
        source.structural_type(),
        function,
        types,
        live,
        operations,
        provenance,
    )
}

pub(super) fn store_field(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::StoreStructuralField {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        return Err(invalid());
    };
    let referent = window_referent(function, live, types, destination, path)?;
    if !window_root(function, prepared, live, destination)
        || !root_window_loan(destination, path, *field)
        || value.access != StructuralAccess::Owned
        || !value.path.is_empty()
    {
        return Err(invalid());
    }
    let (geometry_type, geometry_path) = referent.as_ref().map_or(
        (destination.structural_type(), path.as_slice()),
        |referent| (referent.referent, referent.path.as_slice()),
    );
    let full_path = field_path(types, geometry_type, geometry_path, *field).ok_or_else(invalid)?;
    let (field_type, shape, byte_offset, indices) =
        crate::lowering::structural_layout::runtime_projection(
            geometry_type,
            &full_path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
    // The value must be a complete owned value of the declared field type
    // whose layout is the field's own extent: the reseat writes exactly
    // those bytes. A runtime element step's selector resolves to its
    // dominating scalar source, as the leaf-copy sibling does.
    let indices = super::scalar_sources::runtime_indices(
        indices,
        function,
        &super::scalar_sources::ScalarSources::from(&*live),
    )?;
    let multiplicity = if let Some(home) = live.structural_homes.get(&value.place) {
        if home.structural_type() != field_type
            || home.has_claims()
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
            || home.layout.shape() != shape
        {
            return Err(invalid());
        }
        home.multiplicity()
    } else {
        // An owned argument may still be in ABI fragments. Retain its place,
        // not a fabricated operation-result home; selection already captures
        // the incoming fragments or indirect pointer for this parameter.
        let declaration = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == value.place)
            .ok_or_else(invalid)?;
        let parameter = prepared
            .parameters
            .iter()
            .find(|parameter| parameter.place == value.place)
            .ok_or_else(invalid)?;
        if declaration.access != StructuralAccess::Owned
            || declaration.is_self
            || declaration.structural_type != field_type
            || !declaration.qualifications.is_empty()
            || !declaration.projected_qualifications.is_empty()
            || parameter.shape != shape
        {
            return Err(invalid());
        }
        declaration.multiplicity
    };
    if super::references::contains_reference(types, field_type)
        || multiplicity == StructuralMultiplicity::Linear
    {
        return Err(invalid());
    }
    // The store consumes an affine value as record construction consumes a
    // field operand; an unrestricted value's home stays readable.
    if multiplicity == StructuralMultiplicity::Affine {
        live.structural_homes.remove(&value.place);
    }
    operations.push(TargetUnitOperation::StoreStructuralField {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        value: value.place,
        byte_offset,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Rejoin a returned carrier's exact live producer before using referent
/// geometry. This peels only the explicit boundary; it grants no root window.
pub(super) fn window_referent(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
) -> Result<Option<super::references::WritableReferentDestination>, LoweringError> {
    if !matches!(path.first(), Some(StructuralPathSegment::Referent)) {
        return Ok(None);
    }
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } = root else {
        return Err(invalid());
    };
    let referent =
        super::references::writable_referent_destination(function, live, types, result.place, path)
            .ok_or_else(invalid)?;
    if referent.producer != *producer || referent.result != *result {
        return Err(invalid());
    }
    Ok(Some(referent))
}

/// A whole-root window needs the original exclusive parameter loan. Crossing
/// an owned reference carrier does not grant that authority over its referent.
fn root_window_loan(
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> bool {
    terminal_semantics::structural_window_selects_child(path, field)
        || (path.is_empty()
            && matches!(
                root,
                abstract_operations::StructuralWindowRoot::BorrowedParameter(_)
            ))
}

/// The window root retains its exact mutable-borrowed declaration. An entry
/// parameter has an ABI pointer; a live block arrival has the address forwarded
/// by its incoming edge. Both copies use that pointer without copying the root.
fn window_root(
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    root: &abstract_operations::StructuralWindowRoot,
) -> bool {
    match root {
        abstract_operations::StructuralWindowRoot::BorrowedParameter(root) => {
            root.access == StructuralAccess::MutableBorrow
                && ((function.structural_parameters.contains(root)
                    && super::structural_case::parameter_root(prepared, root.place).is_some_and(
                        |parameter| {
                            parameter.access == StructuralAccess::MutableBorrow
                                && parameter.structural_type == root.structural_type
                        },
                    ))
                    || super::transfers::address_parameter(function, live, root.place)
                        == Some(root))
        }
        // An owned local home's authority is its live home row: the producer
        // completed it under clean transferable custody and nothing else may
        // have claimed it since.
        abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => live
            .structural_homes
            .get(&result.place)
            .is_some_and(|home| {
                home.operation_result() == Some((*producer, result))
                    && matches!(
                        home.multiplicity(),
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && !home.has_claims()
                    && home.qualifications().is_empty()
                    && home.projected_qualifications().is_empty()
            }),
    }
}

/// Resolve the exact window subtree; an index may select a field's parent
/// just as a record field can. The optional field is never inferred from the
/// final path segment.
fn field_path(
    types: &StructuralTypeLookup<'_>,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> Option<Vec<StructuralPathSegment>> {
    terminal_semantics::structural_window_projection(types.values().copied(), root, path, field)
        .map(|projection| projection.path)
}
