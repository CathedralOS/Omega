//! Borrowed-window field replacement realized as two byte copies.
//!
//! `MoveStructuralField` vacates one declared field beneath a mutable-borrowed
//! parameter into a fresh owned result; `StoreStructuralField` reseats exactly
//! that vacancy with an owned subtree of the declared type. Psi checks the
//! window discipline before these rows reach Omega: the
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
//! Resource claims and domain memberships do not change the byte layout.
//! Abstract window admission retains the selected memberships and requires
//! the installed value to supply them; physical transport does not erase or
//! establish them by accepting a carrier's bytes. Reference-bearing storage
//! additionally transfers its exact descriptor roster with the bytes, through
//! the same subtree transfer that construction uses. Backing and parent loan
//! identities remain unchanged; a missing roster still rejects.
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
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralPathSegment,
};

pub(super) fn move_field(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::MoveStructuralField {
        psi_operation,
        result,
        source,
        path,
        field,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = window_referent(function, live, types, source, path)?;
    if !window_root(function, prepared, live, source)
        || !root_window_loan(function, live, source, path, *field)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (geometry_type, geometry_path) = referent
        .as_ref()
        .map_or((source.structural_type(), path.as_slice()), |referent| {
            (referent.referent, referent.path.as_slice())
        });
    let mut path = field_path(types, geometry_type, geometry_path, *field)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if referent.is_some() {
        path.insert(0, StructuralPathSegment::Referent);
    }
    if live
        .references
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut relocations = Vec::new();
    super::references::subtree_leaves(
        function,
        types,
        live,
        &[],
        result.structural_type,
        &StructuralArgument {
            place: source.place(),
            path: path.clone(),
            access: StructuralAccess::Owned,
        },
        &mut relocations,
        &mut BTreeSet::new(),
    )?;
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
    )?;
    super::references::relocate_leaves(live, result.place, relocations);
    Ok(())
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
    let AbstractOperation::StoreStructuralField {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = window_referent(function, live, types, destination, path)?;
    if !window_root(function, prepared, live, destination)
        || !root_window_loan(function, live, destination, path, *field)
        || value.access != StructuralAccess::Owned
        || !value.path.is_empty()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (geometry_type, geometry_path) = referent.as_ref().map_or(
        (destination.structural_type(), path.as_slice()),
        |referent| (referent.referent, referent.path.as_slice()),
    );
    let full_path = field_path(types, geometry_type, geometry_path, *field)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
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
        if home.structural_type() != field_type || home.layout.shape() != shape {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        home.multiplicity()
    } else {
        // An owned argument may still be in ABI fragments. Retain its place,
        // not a fabricated operation-result home; selection already captures
        // the incoming fragments or indirect pointer for this parameter.
        let declaration = function
            .structural_parameter(value.place)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let parameter = prepared
            .parameter(value.place)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if declaration.access != StructuralAccess::Owned
            || declaration.structural_type != field_type
            || parameter.shape != shape
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        declaration.multiplicity
    };
    // The displaced value's ordinary disposition must have vacated every
    // descriptor slot. Installing a replacement cannot silently retire a loan
    // still held in that region or release a disjoint sibling.
    let mut custody_path = full_path;
    if referent.is_some() {
        custody_path.insert(0, StructuralPathSegment::Referent);
    }
    if live
        .references
        .keys()
        .any(|(carrier, path)| *carrier == destination.place() && path.starts_with(&custody_path))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut relocations = Vec::new();
    super::references::subtree_leaves(
        function,
        types,
        live,
        &custody_path,
        field_type,
        value,
        &mut relocations,
        &mut BTreeSet::new(),
    )?;
    super::references::relocate_leaves(live, destination.place(), relocations);
    // The store consumes an owned value as record construction consumes a
    // field operand; an unrestricted value's home stays readable.
    if multiplicity != StructuralMultiplicity::Unrestricted {
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
    if root.parameter().is_some() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let referent =
        super::references::writable_referent_destination(live, types, root.place(), path)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if referent.carrier_type != root.structural_type() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(Some(referent))
}

/// A whole-root window needs the original exclusive parameter loan. Crossing
/// an owned reference carrier does not grant that authority over its referent.
fn root_window_loan(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
) -> bool {
    if terminal_semantics::structural_window_selects_child(path, field) {
        return true;
    }
    if path.is_empty() {
        return matches!(
            root,
            abstract_operations::StructuralWindowRoot::Parameter(_)
        );
    }
    if path.first() != Some(&StructuralPathSegment::Referent) {
        return false;
    }
    let Some(loan) = live.references.get(&(root.place(), Vec::new())) else {
        return false;
    };
    // A retained alias uses its own pointer and live permission, but every
    // possible backing must still owe restoration to exclusive parameter
    // storage. Owning the descriptor never grants that parameter authority.
    terminal_semantics::structural_window_parameter_origins(
        function.structural_parameters.iter().chain(
            function
                .block_entries
                .iter()
                .flat_map(|block| &block.structural_parameters),
        ),
        loan.root.origins().into_iter().map(|origin| origin.root),
    )
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
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => live
            .structural_homes
            .get(&parameter.place)
            .is_some_and(|home| {
                home.origin
                    == target_operations::TargetStructuralHomeOrigin::BlockParameter {
                        block: *block,
                        declaration: parameter.clone(),
                    }
                    && parameter.access == StructuralAccess::Owned
            }),
        abstract_operations::StructuralWindowRoot::Parameter(root) => {
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
        // completed it under transferable custody. Its original claims do not
        // make the home immutable; the window owns the regional transfer.
        abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } => live
            .structural_homes
            .get(&result.place)
            .is_some_and(|home| home.operation_result() == Some((*producer, result))),
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
