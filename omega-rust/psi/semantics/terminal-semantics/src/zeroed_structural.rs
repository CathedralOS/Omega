use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// One scalar leaf cell of a zero-constructible structural type, addressed the
/// way runtime storage keys it: a record field under its parent path, or one
/// primitive-scalar element of a fixed array whose path ends at its index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroedStructuralLeaf {
    RecordField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    ArrayScalar {
        path: Vec<StructuralPathSegment>,
        scalar_type: ScalarType,
    },
}

impl ZeroedStructuralLeaf {
    /// The leaf's canonical static path below the established root.
    pub fn path(&self) -> &[StructuralPathSegment] {
        match self {
            ZeroedStructuralLeaf::RecordField { path, .. }
            | ZeroedStructuralLeaf::ArrayScalar { path, .. } => path,
        }
    }

    /// The leaf's plain scalar storage type.
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            ZeroedStructuralLeaf::RecordField { scalar_type, .. }
            | ZeroedStructuralLeaf::ArrayScalar { scalar_type, .. } => *scalar_type,
        }
    }
}

/// Resolve whether a structural type's whole storage may begin zero: every
/// leaf a plain primitive scalar and every aggregate a record or fixed array.
/// References, views, byte sequences, sums, bounded integers, and erased
/// members have no universally valid zero spelling (a bounded integer's
/// restriction may exclude 0), so they reject. Unknown, cyclic, or recursive
/// membership also rejects — the worklist bounds the walk at the module's
/// declaration count.
pub fn zeroed_structural_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    zeroed_structural_leaves(types, root).is_some()
}

/// Enumerate every scalar leaf a zeroed establishment of `root` must
/// materialize, or `None` when the shape is not zero-constructible (the same
/// admission rule as [`zeroed_structural_shape`]). Record fields key by their
/// parent path and field id; fixed-array elements key by the full indexed
/// path.
pub fn zeroed_structural_leaves<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    let declarations: Vec<&StructuralTypeDeclaration> = types.collect();
    let bound = declarations.len() + 1;
    let mut leaves = Vec::new();
    // (type id, path to its runtime place root)
    let mut pending = vec![(root, Vec::new())];
    let mut visited_cells = 0usize;
    while let Some((current, path)) = pending.pop() {
        visited_cells += 1;
        // Bounded by declarations × membership: a recursion that re-enters an
        // ancestor would only enumerate distinct (type, path) cells, and the
        // declaration set caps each member's distinct type ids.
        if visited_cells > bound.saturating_mul(bound).max(1024) {
            return None;
        }
        let Some(declaration) = declarations
            .iter()
            .find(|declaration| declaration.id == current)
        else {
            return None;
        };
        match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                    path,
                    scalar_type: *scalar_type,
                });
            }
            StructuralTypeShape::FixedArray { element, length } => {
                let extent = usize::try_from(*length).ok()?;
                for index in 0..extent {
                    let mut element_path = path.clone();
                    element_path.push(StructuralPathSegment::FixedIndex(index as u64));
                    pending.push((*element, element_path));
                }
            }
            StructuralTypeShape::Record { fields } => {
                for field in fields {
                    match &field.field_type {
                        StructuralFieldType::Scalar(scalar_type) => {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: *scalar_type,
                            });
                        }
                        StructuralFieldType::Structural(member) => {
                            let mut member_path = path.clone();
                            member_path.push(StructuralPathSegment::Field(field.identity.clone()));
                            pending.push((*member, member_path));
                        }
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
    }
    Some(leaves)
}
