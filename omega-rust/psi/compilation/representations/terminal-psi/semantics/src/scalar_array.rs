use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

/// Recover one row-major scalar operand's exact nested array coordinate.
/// Empty, cyclic and nonprimitive carriers have no such initialized leaf.
pub fn scalar_array_leaf_path<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    mut ordinal: u64,
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let (_, count) = scalar_array_leaf_declaration(types.clone(), root)?;
    if ordinal >= count {
        return None;
    }
    let mut dimensions = Vec::new();
    let mut current = root;
    loop {
        match types.clone().find(|row| row.id == current)?.shape {
            StructuralTypeShape::FixedArray { element, length } => {
                dimensions.push(length);
                current = element;
            }
            StructuralTypeShape::PrimitiveScalar(_) => break,
            _ => return None,
        }
    }
    let mut path = Vec::with_capacity(dimensions.len());
    for length in dimensions.into_iter().rev() {
        path.push(terminal_psi::StructuralPathSegment::FixedIndex(
            ordinal.checked_rem(length)?,
        ));
        ordinal = ordinal.checked_div(length)?;
    }
    path.reverse();
    Some(path)
}

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
