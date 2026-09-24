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
//! it needs no second replay here.
use super::{AbstractOperationPlan, PsiOptimizationFunction};
use crate::LegalizationError;
use crate::structural_inputs::structural_reference_input;
use calling_conventions::ValueShape;
use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
};

/// The resolved bytes one window names beneath its root.
pub(in crate::legalization) struct WindowExtent {
    /// The spelled path extended by the field's segment: the projection a
    /// `StructuralLeafCopy` row retains for the move.
    pub(in crate::legalization) path: Vec<StructuralPathSegment>,
    pub(in crate::legalization) field_type: StructuralTypeId,
    pub(in crate::legalization) byte_offset: u32,
    pub(in crate::legalization) shape: ValueShape,
}

/// Resolve the window `root` + `path` + `field`. The root must be this
/// function's own mutable-borrowed parameter row, exactly as declared; the
/// field must be a non-erased plain structural member of the record the path
/// reaches. A reference-bearing subtree carries loan custody a byte copy
/// cannot relocate, and a spelled window path never traverses a runtime index.
pub(in crate::legalization) fn extent(
    function: &PsiOptimizationFunction,
    root: &StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    plan: &AbstractOperationPlan,
) -> Result<WindowExtent, LegalizationError> {
    let invalid = LegalizationError::custody;
    if root.access != StructuralAccess::MutableBorrow
        || !function.structural_parameters.contains(root)
        || !function.entry_claims.is_empty()
    {
        return Err(invalid());
    }
    let types = &plan.structural_types;
    let (parent, _) = structural_reference_input::project(root.structural_type, path, types)
        .ok_or_else(invalid)?;
    let StructuralTypeShape::Record { fields } = &types
        .iter()
        .find(|declaration| declaration.id == parent)
        .ok_or_else(invalid)?
        .shape
    else {
        return Err(invalid());
    };
    let declaration = fields
        .iter()
        .find(|declaration| declaration.id == field && !declaration.relevance.is_erased())
        .ok_or_else(invalid)?;
    let StructuralFieldType::Structural(field_type) = declaration.field_type else {
        return Err(invalid());
    };
    let mut full = path.to_vec();
    full.push(StructuralPathSegment::Field(declaration.identity.clone()));
    let (endpoint, byte_offset, indices) =
        structural_reference_input::leaf_copy_projection(root.structural_type, &full, types)
            .ok_or_else(invalid)?;
    if endpoint != field_type
        || !indices.is_empty()
        || super::reference_custody::contains_reference(types, field_type)
    {
        return Err(invalid());
    }
    let shape = structural_reference_input::shape(field_type, types).ok_or_else(invalid)?;
    Ok(WindowExtent {
        path: full,
        field_type,
        byte_offset,
        shape,
    })
}

/// The extent a `MoveStructuralField` copies out: the window's field, whose
/// declared type the fresh result carries under exact-once custody with no
/// shared or linear multiplicity, qualification or claim.
pub(in crate::legalization) fn moved(
    function: &PsiOptimizationFunction,
    root: &StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<WindowExtent, LegalizationError> {
    let extent = extent(function, root, path, field, plan)?;
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
