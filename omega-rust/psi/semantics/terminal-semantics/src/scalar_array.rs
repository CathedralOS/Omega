use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

/// Resolve the exact primitive leaf and total leaf count of a fixed array.
/// Unknown/cyclic/nonprimitive shapes reject even below an empty dimension.
/// Zero dimensions suppress product overflow, but never suppress type checking.
pub fn scalar_array_leaf_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(ScalarType, u64)> {
    let mut current = root;
    let mut leaf_count = Some(1_u64);
    let mut empty = false;
    let mut array = false;
    for _ in 0..types.clone().count() {
        match types
            .clone()
            .find(|declaration| declaration.id == current)?
            .shape
        {
            StructuralTypeShape::FixedArray { element, length } => {
                array = true;
                empty |= length == 0;
                leaf_count = leaf_count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(scalar_type) if array => {
                return Some((scalar_type, if empty { 0 } else { leaf_count? }));
            }
            _ => return None,
        }
    }
    None
}
