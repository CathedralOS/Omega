//! Borrowed-window moves and stores rejoin one independently resolved field
//! extent.
//!
//! Target lowering realizes `MoveStructuralField` as the byte copy a
//! `StructuralLeafCopy` row performs, over the spelled window path extended by
//! the vacated field's own segment, and `StoreStructuralField` as the inverse
//! copy (`TargetUnitOperation::StoreStructuralField`). This module recomputes
//! that extent from the abstract row and the plan's declarations alone, so the
//! target rejoin, the legalized projection and the legalized replay each
//! compare the retained offset and shape with the declared field instead of
//! trusting the target row. The checked window discipline itself (the move
//! opens restoration debt, nothing observes the hole, the store closes it)
//! belongs to Psi and the emitted window's sequencing;
//! it needs no second replay here. A root window (`field: None`, empty path)
//! names the whole referent of a borrowed-parameter loan at offset zero.
//! Membership transport is checked against exact selected declaration rows
//! before physical lowering. Qualifications add no bytes and cannot replace
//! the independent origin, projection, layout or reference-custody checks here.
use super::{AbstractOperationPlan, PsiOptimizationFunction};
use crate::LegalizationError;
use crate::structural_inputs::structural_reference_input::{self, RuntimeElement};
use calling_conventions::ValueShape;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralOperationResult, StructuralPathSegment};

/// The resolved bytes one window names beneath its root.
pub(in crate::legalization) struct WindowExtent {
    /// The projection the byte copy realizes: the spelled path extended by
    /// the field's segment for a field leaf, or the spelled path unchanged
    /// for an element leaf (it already ends in the element selection).
    pub(in crate::legalization) path: Vec<StructuralPathSegment>,
    pub(in crate::legalization) field_type: StructuralTypeId,
    pub(in crate::legalization) byte_offset: u32,
    pub(in crate::legalization) shape: ValueShape,
    /// The runtime elements `path` traverses, in path order — empty for a
    /// fully static route.
    pub(in crate::legalization) elements: Vec<RuntimeElement>,
}

/// Resolve the window `root` + `path` + `field`. The root must be this
/// function's own mutable-borrowed entry or block parameter row, exactly as declared; the
/// optional field must be a non-erased plain structural member of the record
/// the path reaches. Without a field the path names the whole selected subtree.
/// Reference-bearing subtrees retain their independently reconstructed leaf
/// roster in `reference_custody::apply`; geometry does not grant that custody.
pub(in crate::legalization) fn extent(
    function: &PsiOptimizationFunction,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<WindowExtent, LegalizationError> {
    let referent = if matches!(path.first(), Some(StructuralPathSegment::Referent)) {
        if root.parameter().is_some() {
            return Err(LegalizationError::custody());
        }
        let referent = super::reference_custody::writable_referent_destination(
            function,
            custody,
            &plan.structural_types,
            root.place(),
            path,
        )
        .ok_or(LegalizationError::custody())?;
        if referent.carrier_type != root.structural_type()
            || root.multiplicity() != terminal_psi::StructuralMultiplicity::Affine
        {
            return Err(LegalizationError::custody());
        }
        Some(referent)
    } else {
        None
    };
    let root_type = match root {
        abstract_operations::StructuralWindowRoot::OwnedBlockParameter { block, parameter } => {
            if parameter.access != StructuralAccess::Owned
                || !function.blocks.iter().any(|candidate| {
                    candidate.id == *block && candidate.structural_parameters.contains(parameter)
                })
            {
                return Err(LegalizationError::custody());
            }
            parameter.structural_type
        }
        abstract_operations::StructuralWindowRoot::Parameter(root) => {
            if root.access != StructuralAccess::MutableBorrow
                || !(function.structural_parameters.contains(root)
                    || (function
                        .blocks
                        .iter()
                        .any(|block| block.structural_parameters.contains(root))
                        && abstract_operations::control_flow::address_joins::is_address_join_in(
                            root,
                            &plan.structural_types,
                        )))
                || function
                    .entry_claim_declarations
                    .iter()
                    .any(|claim| claim.input == root.place)
            {
                return Err(LegalizationError::custody());
            }
            root.structural_type
        }
        // An owned completed local home carries no borrowed-window custody;
        // its authority was admitted upstream from the completing producer.
        abstract_operations::StructuralWindowRoot::OwnedResult { result, .. } => {
            result.structural_type
        }
    };
    let (root_type, path) = referent.as_ref().map_or((root_type, path), |referent| {
        (referent.referent, referent.path.as_slice())
    });
    let types = &plan.structural_types;
    // Reference crossings retain whole-root authority requirements; only a
    // following field or element selects a child of that referent.
    if !terminal_semantics::structural_window_selects_child(path, field) {
        let authenticated = if referent.is_some() {
            custody.leaf(root.place(), &[]).is_some_and(|loan| {
                terminal_semantics::structural_window_parameter_origins(
                    function.structural_parameters.iter().chain(
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    ),
                    loan.root.origins().into_iter().map(|origin| origin.root),
                )
            })
        } else {
            path.is_empty()
                && matches!(
                    root,
                    abstract_operations::StructuralWindowRoot::Parameter(_)
                )
        };
        if !authenticated {
            return Err(LegalizationError::custody());
        }
    }
    let projection =
        terminal_semantics::structural_window_projection(types.iter(), root_type, path, field)
            .ok_or(LegalizationError::custody())?;
    let mut full = projection.path;
    let field_type = projection.structural_type;
    let (endpoint, byte_offset, elements) =
        structural_reference_input::runtime_projection(root_type, &full, types)
            .ok_or(LegalizationError::custody())?;
    if endpoint != field_type {
        return Err(LegalizationError::custody());
    }
    let shape =
        structural_reference_input::shape(field_type, types).ok_or(LegalizationError::custody())?;
    if referent.is_some() {
        full.insert(0, StructuralPathSegment::Referent);
    }
    Ok(WindowExtent {
        path: full,
        field_type,
        byte_offset,
        shape,
        elements,
    })
}

/// The extent a `MoveStructuralField` copies out: the window's field, whose
/// declared type the fresh result carries under exact-once custody. Resource
/// claims retain that result declaration; they do not alter the copied extent.
pub(in crate::legalization) fn moved(
    function: &PsiOptimizationFunction,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<WindowExtent, LegalizationError> {
    let extent = extent(function, root, path, field, plan, custody)?;
    if result.structural_type != extent.field_type {
        return Err(LegalizationError::custody());
    }
    Ok(extent)
}
