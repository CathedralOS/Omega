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
//! trusting the target row. The verified window discipline itself (the move
//! opens restoration debt, nothing observes the hole, the store closes it)
//! was replayed by the Terminal verifier and optimized-unit ownership replay;
//! it needs no second replay here. A root window (`field: None`, empty path)
//! names the whole referent of a borrowed-parameter loan at offset zero.
use super::{AbstractOperationPlan, PsiOptimizationFunction};
use crate::LegalizationError;
use crate::structural_inputs::structural_reference_input::{self, RuntimeElement};
use calling_conventions::ValueShape;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
};

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
/// A reference-bearing
/// subtree carries loan custody a byte copy cannot relocate.
pub(in crate::legalization) fn extent(
    function: &PsiOptimizationFunction,
    root: &abstract_operations::StructuralWindowRoot,
    path: &[StructuralPathSegment],
    field: Option<StructuralFieldId>,
    plan: &AbstractOperationPlan,
    custody: &super::reference_custody::Custody,
) -> Result<WindowExtent, LegalizationError> {
    let invalid = LegalizationError::custody;
    let referent = if matches!(path.first(), Some(StructuralPathSegment::Referent)) {
        let abstract_operations::StructuralWindowRoot::OwnedResult { producer, result } = root
        else {
            return Err(invalid());
        };
        let referent = super::reference_custody::writable_referent_destination(
            custody,
            function,
            &plan.structural_types,
            root.place(),
            path,
        )
        .ok_or_else(invalid)?;
        if referent.producer != *producer || referent.result != *result {
            return Err(invalid());
        }
        Some(referent)
    } else {
        None
    };
    let root_type = match root {
        abstract_operations::StructuralWindowRoot::BorrowedParameter(root) => {
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
                || !function.entry_claims.is_empty()
            {
                return Err(invalid());
            }
            root.structural_type
        }
        // An owned completed local home carries no borrowed-window custody;
        // its authority was admitted upstream from the completing producer.
        abstract_operations::StructuralWindowRoot::OwnedResult { result, .. } => {
            if !matches!(
                result.multiplicity,
                StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
            ) || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            result.structural_type
        }
    };
    let (root_type, path) = referent.as_ref().map_or((root_type, path), |referent| {
        (referent.referent, referent.path.as_slice())
    });
    let types = &plan.structural_types;
    // Only the empty complete projection replaces the whole root. Element
    // and nested-field windows retain their enclosing storage's authority.
    if field.is_none()
        && path.is_empty()
        && !matches!(
            root,
            abstract_operations::StructuralWindowRoot::BorrowedParameter(_)
        )
    {
        return Err(invalid());
    }
    let projection =
        terminal_semantics::structural_window_projection(types.iter(), root_type, path, field)
            .ok_or_else(invalid)?;
    let mut full = projection.path;
    let field_type = projection.structural_type;
    let (endpoint, byte_offset, elements) =
        structural_reference_input::runtime_projection(root_type, &full, types)
            .ok_or_else(invalid)?;
    if endpoint != field_type || super::reference_custody::contains_reference(types, field_type) {
        return Err(invalid());
    }
    let shape = structural_reference_input::shape(field_type, types).ok_or_else(invalid)?;
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
/// declared type the fresh result carries under exact-once custody with no
/// shared or linear multiplicity, qualification or claim.
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
    if result.structural_type != extent.field_type
        || !matches!(
            result.multiplicity,
            StructuralMultiplicity::Affine | StructuralMultiplicity::Unrestricted
        )
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    Ok(extent)
}
