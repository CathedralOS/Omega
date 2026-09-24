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
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::StructuralFieldId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeShape,
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
    if !window_root(function, prepared, source)
        || super::references::contains_reference(types, result.structural_type)
    {
        return Err(invalid());
    }
    let path = field_path(types, source.structural_type, path, *field).ok_or_else(invalid)?;
    super::leaf_copy::copy_extent(
        *psi_operation,
        result,
        source.place,
        &path,
        source.structural_type,
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
    if !window_root(function, prepared, destination)
        || value.access != StructuralAccess::Owned
        || !value.path.is_empty()
    {
        return Err(invalid());
    }
    let full_path =
        field_path(types, destination.structural_type, path, *field).ok_or_else(invalid)?;
    let (field_type, shape, byte_offset, indices) =
        crate::lowering::structural_layout::leaf_copy_projection(
            destination.structural_type,
            &full_path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?;
    // The value must be a complete owned home of the declared field type
    // whose layout is the field's own extent: the reseat writes exactly
    // those bytes. A spelled window path has no runtime index to scale.
    let value_home = live
        .structural_homes
        .get(&value.place)
        .cloned()
        .ok_or_else(invalid)?;
    if !indices.is_empty()
        || super::references::contains_reference(types, field_type)
        || value_home.structural_type() != field_type
        || value_home.multiplicity() == StructuralMultiplicity::Linear
        || value_home.has_claims()
        || !value_home.qualifications().is_empty()
        || !value_home.projected_qualifications().is_empty()
        || value_home.layout.shape() != shape
    {
        return Err(invalid());
    }
    // The store consumes an affine value as record construction consumes a
    // field operand; an unrestricted value's home stays readable.
    if value_home.multiplicity() == StructuralMultiplicity::Affine {
        live.structural_homes.remove(&value.place);
    }
    operations.push(TargetUnitOperation::StoreStructuralField {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        value_home,
        byte_offset,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// The window root is this function's own mutable-borrowed parameter row,
/// exactly as declared, and its referent pointer is part of the prepared
/// signature: both copies address the field through that pointer.
fn window_root(
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    root: &StructuralParameterDeclaration,
) -> bool {
    root.access == StructuralAccess::MutableBorrow
        && function.structural_parameters.contains(root)
        && super::structural_case::parameter_root(prepared, root.place).is_some_and(|parameter| {
            parameter.access == StructuralAccess::MutableBorrow
                && parameter.structural_type == root.structural_type
        })
}

/// The spelled window path extended by the vacated field's own segment, so
/// the byte copy resolves the field's offset and extent like any projection.
fn field_path(
    types: &StructuralTypeLookup<'_>,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
) -> Option<Vec<StructuralPathSegment>> {
    let parent = types.subtree(root, path)?;
    let StructuralTypeShape::Record { fields } = &types.get(&parent)?.shape else {
        return None;
    };
    let declaration = fields
        .iter()
        .find(|declaration| declaration.id == field && !declaration.relevance.is_erased())?;
    let mut full = path.to_vec();
    full.push(StructuralPathSegment::Field(declaration.identity.clone()));
    Some(full)
}
