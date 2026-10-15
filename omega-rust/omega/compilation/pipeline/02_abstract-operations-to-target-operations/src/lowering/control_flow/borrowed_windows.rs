//! Borrowed structural moves and stores realized as whole-value byte copies.
//! An intact freely discardable replacement emits only the store. Its retained
//! disposition grants write-only destination access without manufacturing an
//! extraction or reading the displaced value.
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
//! the same subtree transfer that construction uses. Possible backing origins
//! follow current descriptor storage; permission remains Psi's responsibility.
//!
//! A root window (`field: None`, empty path) is whole-value assignment
//! through an exclusive loan: both copies span the whole referent at offset
//! zero beneath the loan's pointer. A captured mutable carrier retains its
//! actual live owned or exclusive-ingress backing; descriptor ownership is
//! not permission to write the referent.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::StructuralFieldId;
use std::collections::BTreeSet;
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
    let referent = window_referent(
        function,
        live,
        types,
        source,
        path,
        StructuralAccess::MutableBorrow,
    )?;
    if !window_root(
        function,
        prepared,
        live,
        source,
        StructuralAccess::MutableBorrow,
    ) || !root_window_loan(
        function,
        live,
        types,
        source,
        path,
        *field,
        StructuralAccess::MutableBorrow,
    ) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (geometry_type, geometry_path) = referent
        .as_ref()
        .map_or((source.structural_type(), path.as_slice()), |referent| {
            (referent.referent, referent.path.as_slice())
        });
    let mut path = field_path(types, geometry_type, geometry_path, *field)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if let Some(referent) = &referent {
        let mut full = referent.carrier_path.clone();
        full.push(StructuralPathSegment::Referent);
        full.extend(path);
        path = full;
    }
    if live
        .references
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut relocations = Vec::new();
    let mut moved_leaves = BTreeSet::new();
    let (storage, storage_path) =
        if super::references::contains_reference(types, result.structural_type) {
            super::references::window_storage(function, types, live, source.place(), &path)?
        } else {
            (source.place(), path.clone())
        };
    super::references::subtree_leaves(
        function,
        types,
        live,
        &[],
        result.structural_type,
        &StructuralArgument {
            place: storage,
            path: storage_path,
            access: StructuralAccess::Owned,
        },
        true,
        &mut relocations,
        &mut moved_leaves,
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
    super::references::relocate_leaves(live, result.place, relocations, moved_leaves);
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
        disposition,
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = window_referent(
        function,
        live,
        types,
        destination,
        path,
        disposition.required_access(),
    )?;
    if !window_root(
        function,
        prepared,
        live,
        destination,
        disposition.required_access(),
    ) || !root_window_loan(
        function,
        live,
        types,
        destination,
        path,
        *field,
        disposition.required_access(),
    ) || value.access != StructuralAccess::Owned
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
        crate::lowering::structural_layout::reference_referent_projection(
            destination.structural_type(),
            &{
                let mut full = path.clone();
                full.extend_from_slice(&full_path[geometry_path.len()..]);
                full
            },
            disposition.required_access(),
            None,
            types,
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
    let (value_type, multiplicity) =
        super::aggregate_results::value_input(function, types, live, value)?;
    if value_type != field_type
        || super::aggregate_results::value_home_layout(value_type, types)?.shape() != shape
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // The displaced value's ordinary disposition must have vacated every
    // descriptor slot. Installing a replacement cannot silently retire a loan
    // still held in that region or release a disjoint sibling.
    let mut custody_path = full_path;
    if let Some(referent) = &referent {
        let mut full = referent.carrier_path.clone();
        full.push(StructuralPathSegment::Referent);
        full.extend(custody_path);
        custody_path = full;
    }
    let (storage, custody_path) = if super::references::contains_reference(types, field_type) {
        super::references::window_storage(
            function,
            types,
            live,
            destination.place(),
            &custody_path,
        )?
    } else {
        (destination.place(), custody_path)
    };
    if *disposition == terminal_psi::StructuralStoreDisposition::ReplaceDiscardable {
        // Source-free admission proved exact shared-descriptor disposal and
        // child closure. End those resident keys before installing new ones;
        // code generation still emits only the incoming value's store.
        super::references::discard_subtree(function, types, live, storage, &custody_path)?;
    }
    if live
        .references
        .keys()
        .any(|(carrier, path)| *carrier == storage && path.starts_with(&custody_path))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut relocations = Vec::new();
    let mut moved_leaves = BTreeSet::new();
    super::references::subtree_leaves(
        function,
        types,
        live,
        &custody_path,
        field_type,
        value,
        multiplicity != StructuralMultiplicity::Unrestricted,
        &mut relocations,
        &mut moved_leaves,
    )?;
    super::references::relocate_leaves(live, storage, relocations, moved_leaves);
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
    required_access: StructuralAccess,
) -> Result<Option<super::references::ReferentProjection>, LoweringError> {
    if !path.contains(&StructuralPathSegment::Referent) {
        return Ok(None);
    }
    let referent = super::references::window_projection(
        function,
        live,
        types,
        root.place(),
        path,
        required_access,
    )
    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if referent.carrier_type != root.structural_type() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(Some(referent))
}

/// The live captured loan, not ownership of its descriptor, grants access to
/// the backing. Owned local storage and exclusive ingress retain the same
/// replacement operation; every possible origin must have actual custody.
fn root_window_loan(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    required_access: StructuralAccess,
) -> bool {
    if terminal_semantics::structural_window_selects_child(path, field) {
        return true;
    }
    if path.is_empty() {
        return root.parameter().is_some();
    }
    if !path.contains(&StructuralPathSegment::Referent) {
        return false;
    }
    let Ok(descriptors) =
        super::references::window_descriptors(function, types, live, root.place(), path)
    else {
        return false;
    };
    // A retained alias uses its own pointer and live permission, but every
    // possible backing must still have live owned or exclusive storage
    // authority. Owning the descriptor never grants referent authority.
    terminal_semantics::structural_window_origins(
        function.structural_parameters.iter().chain(
            function
                .block_entries
                .iter()
                .flat_map(|block| &block.structural_parameters),
        ),
        descriptors
            .into_iter()
            .flat_map(|leaf| leaf.root.origins())
            .map(|origin| origin.root),
        required_access,
        |place| {
            live.structural_homes.get(&place).is_some_and(|home| {
                home.access() == StructuralAccess::Owned
                    && home.multiplicity() != StructuralMultiplicity::Linear
                    && match &home.origin {
                        target_operations::TargetStructuralHomeOrigin::OperationResult {
                            result,
                            ..
                        } => {
                            // Value memberships are restored by the admitted
                            // structural store. They do not remove the live
                            // owner's storage authority; claims stay separate.
                            result.claims.is_empty()
                        }
                        target_operations::TargetStructuralHomeOrigin::BlockParameter {
                            ..
                        } => true,
                    }
            })
        },
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
    required_access: StructuralAccess,
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
            required_access.admits(root.access)
                && ((function.structural_parameters.contains(root)
                    && prepared.parameter(root.place).is_some_and(|parameter| {
                        parameter.access == root.access
                            && parameter.structural_type == root.structural_type
                    }))
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
