//! Shared lowering for one field-projected structural call argument.

use super::super::shared::*;
use super::super::structural_layout::resolve_structural_field_path;
use psi_terminal::{StructuralArgument, StructuralParameterDeclaration};

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    caller: MachineId,
    callee: MachineId,
    argument: &StructuralArgument,
    callee_parameter: &StructuralParameterDeclaration,
    destination: &ValuePlacement,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let source = parameters_by_place.get(&argument.place).copied().ok_or(
        LoweringError::UnknownStructuralArgumentPlace {
            machine: caller,
            place: argument.place,
        },
    )?;
    if argument.path.is_empty()
        || argument
            .path
            .iter()
            .any(|segment| !matches!(segment, StructuralPathSegment::Field(_)))
    {
        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
            callee,
            place: argument.place,
        });
    }
    let (projected_type, projected_shape, source_byte_offset) = resolve_structural_field_path(
        source.structural_type,
        &argument.path,
        structural_types,
        shape_cache,
        active,
    )
    .map_err(|_| LoweringError::StructuralCallArgumentTypeMismatch {
        callee,
        place: argument.place,
    })?;
    if projected_type != callee_parameter.structural_type
        || projected_shape != destination.shape
        || u32::from(projected_shape.byte_size)
            .checked_add(source_byte_offset)
            .is_none_or(|end| end > u32::from(source.shape.byte_size))
    {
        return Err(LoweringError::StructuralCallArgumentTypeMismatch {
            callee,
            place: argument.place,
        });
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: source.structural_type,
        structural_type: projected_type,
        shape: projected_shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        source: source.placement.clone(),
        destination: destination.clone(),
    })
}
