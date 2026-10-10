use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

/// Resolve the exact primitive leaf and total leaf count of a fixed array.
/// Unknown/cyclic/nonprimitive shapes reject even below an empty dimension.
/// Zero dimensions suppress product overflow, but never suppress type checking.
pub fn scalar_array_leaf_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(ScalarType, u64)> {
    scalar_array_leaf_declaration(types, root).map(|(declaration, count)| {
        let StructuralTypeShape::PrimitiveScalar(scalar) = declaration.shape else {
            unreachable!()
        };
        (scalar, count)
    })
}

/// Exact declaration of every initialized scalar leaf, independent of layout.
pub fn scalar_array_leaf_declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<(&'types StructuralTypeDeclaration, u64)> {
    let mut current = root;
    let mut leaf_count = Some(1_u64);
    let mut empty = false;
    let mut array = false;
    for _ in 0..types.clone().count() {
        let declaration = types
            .clone()
            .find(|declaration| declaration.id == current)?;
        match declaration.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                array = true;
                empty |= length == 0;
                leaf_count = leaf_count.and_then(|count| count.checked_mul(length));
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(_) if array => {
                return Some((declaration, if empty { 0 } else { leaf_count? }));
            }
            _ => return None,
        }
    }
    None
}
