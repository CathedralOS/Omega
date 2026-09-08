//! Exact projected exclusive references retain their incoming root pointer.

use super::*;
use terminal_psi::{StructuralMultiplicity, StructuralParameterDeclaration};

pub(super) fn argument(
    semantic: &StructuralArgument,
    caller: &PsiOptimizationFunction,
    destination: &StructuralParameterDeclaration,
    call: &CallPlan,
    scalar_count: usize,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
) -> Result<TargetStructuralArgument, LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let source = caller
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == semantic.place)
        .ok_or(invalid.clone())?;
    let target_caller = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or(invalid.clone())?;
    let parameter = super::super::structural_parameters(target_caller)
        .ok_or(invalid.clone())?
        .iter()
        .find(|parameter| parameter.place == semantic.place)
        .ok_or(invalid.clone())?;
    let types = &plan.structural_types;
    let root_shape = crate::structural_reference_input::shape(source.structural_type, types)
        .ok_or(invalid.clone())?;
    let (structural_type, source_byte_offset) =
        crate::structural_reference_input::project(source.structural_type, &semantic.path, types)
            .ok_or(invalid.clone())?;
    let referent =
        crate::structural_reference_input::shape(structural_type, types).ok_or(invalid.clone())?;
    let shape = ValueShape::borrowed_reference(referent.byte_size, referent.alignment);
    if semantic.place != source.place
        || destination.access != semantic.access
        || !matches!(
            (source.access, semantic.access),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ) | (
                StructuralAccess::WriteOnlyBorrow,
                StructuralAccess::WriteOnlyBorrow
            ) | (
                StructuralAccess::SharedBorrow,
                StructuralAccess::SharedBorrow
            )
        )
        || source.multiplicity != StructuralMultiplicity::Unrestricted
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !source.qualifications.is_empty()
        || !source.projected_qualifications.is_empty()
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || structural_type != destination.structural_type
        || parameter.place != source.place
        || parameter.structural_type != source.structural_type
        || parameter.access != source.access
        || parameter.multiplicity != source.multiplicity
        || !parameter.projected_qualifications.is_empty()
        || parameter.shape
            != ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment)
        || call
            .parameters
            .get(scalar_count)
            .is_none_or(|placement| placement.shape != shape)
    {
        return Err(invalid);
    }
    Ok(TargetStructuralArgument {
        place: semantic.place,
        access: semantic.access,
        path: semantic.path.clone(),
        root_structural_type: source.structural_type,
        structural_type,
        shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        source: parameter.placement.clone().into(),
        destination: call.parameters[scalar_count].clone(),
    })
}
