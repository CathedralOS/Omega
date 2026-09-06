//! Reconstruct an owned affine argument's physical path before assigning it.

use super::*;

pub(super) fn exact(
    body: &target_operations::TargetUnitBody,
    source: &target_operations::TargetStructuralParameter,
    argument: &target_operations::TargetStructuralArgument,
) -> bool {
    let Some(declarations) = structural_scalar::declaration_map(&body.structural_types) else {
        return false;
    };
    let Some((leaf_type, leaf_shape, byte_offset)) = structural_scalar::resolve_projection_path(
        source.structural_type,
        &argument.path,
        &declarations,
    ) else {
        return false;
    };
    let Some(root_shape) =
        structural_scalar::structural_value_shape(source.structural_type, &declarations)
    else {
        return false;
    };
    let metadata = match declarations
        .get(&source.structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(terminal_psi::StructuralTypeShape::FixedArray { element, length }) => {
            let Some(element_shape) =
                structural_scalar::structural_value_shape(*element, &declarations)
            else {
                return false;
            };
            let Some(stride) = u32::from(element_shape.byte_size)
                .checked_next_multiple_of(u32::from(element_shape.alignment))
            else {
                return false;
            };
            (Some(*length), Some(stride))
        }
        _ => (None, None),
    };
    source.access == terminal_psi::StructuralAccess::Owned
        && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && source.projected_qualifications.is_empty()
        && source.shape == root_shape
        && argument.access == terminal_psi::StructuralAccess::Owned
        && argument.root_structural_type == source.structural_type
        && argument.structural_type == leaf_type
        && argument.shape == leaf_shape
        && argument.source_byte_offset == byte_offset
        && (argument.fixed_array_length, argument.element_stride) == metadata
        && byte_offset
            .checked_add(u32::from(leaf_shape.byte_size))
            .is_some_and(|end| end <= u32::from(root_shape.byte_size))
}
