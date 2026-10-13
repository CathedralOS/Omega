//! Type geometry for returned provenance. Element families describe a possible
//! subobject, never an evaluated selector or permission to access one. Keep the
//! executable path walkers unchanged so a catalog cannot supply an address.

use semantic_vocabulary::{IntegerSign, ScalarType, StructuralTypeId};
use terminal_psi::{
    ReferenceOriginProjection, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// The exact supplying permission and the relation below it. All consumers use
/// this split; none evaluate a callee's selector in the caller's value namespace.
pub struct ReferenceOriginBinding<'origin> {
    pub source: terminal_psi::StructuralArgument,
    pub suffix: &'origin [ReferenceOriginProjection],
    pub endpoint: StructuralTypeId,
}

pub fn bind_reference_origin<'types, 'origin>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    origin: &'origin terminal_psi::ReferenceOriginSource,
) -> Result<ReferenceOriginBinding<'origin>, &'static str> {
    use ReferenceOriginProjection::{Element, Exact};
    use StructuralPathSegment::{FixedIndex, Referent};
    let endpoint = reference_origin_type(types.clone(), root, &origin.path)
        .ok_or("returned origin has no typed endpoint")?;
    let selection = origin
        .path
        .iter()
        .position(|projection| matches!(projection, Element | Exact(FixedIndex(_))));
    let last_crossing = origin
        .path
        .iter()
        .rposition(|projection| *projection == Exact(Referent));
    let split = selection.map_or(origin.path.len(), |selection| {
        last_crossing.map_or(selection, |crossing| selection.max(crossing + 1))
    });
    let prefix = origin.path[..split]
        .iter()
        .map(|projection| match projection {
            Exact(segment) => Ok(segment.clone()),
            Element => Err("returned origin needs a captured stored-descriptor identity"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let source = terminal_psi::StructuralArgument {
        place: origin.place,
        path: crate::reference_source_storage_path(types, root, &prefix).to_vec(),
        access: origin.access,
    };
    Ok(ReferenceOriginBinding {
        source,
        suffix: &origin.path[split..],
        endpoint,
    })
}

/// Conservative aliasing between provenance coordinates. Families can overlap
/// any element; a view or differing overlay case cannot establish disjointness.
/// These paths are never used to perform an executable projection.
pub fn reference_origin_paths_may_overlap(
    left: &[ReferenceOriginProjection],
    right: &[ReferenceOriginProjection],
) -> bool {
    use ReferenceOriginProjection::{Element, Exact};
    for (left, right) in left.iter().zip(right) {
        match (left, right) {
            (Exact(StructuralPathSegment::RepresentationView { .. }), _)
            | (_, Exact(StructuralPathSegment::RepresentationView { .. })) => return true,
            (
                Exact(StructuralPathSegment::Case(left)),
                Exact(StructuralPathSegment::Case(right)),
            ) if left != right => return true,
            (Element, _) | (_, Element) => {}
            (Exact(left), Exact(right))
                if !crate::structural_paths_may_overlap(
                    std::slice::from_ref(left),
                    std::slice::from_ref(right),
                ) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// Resolve live provenance that may still carry a captured runtime selector.
/// Forget only its value for type geometry, never its formation or address.
/// Declaration decoding uses the stricter `reference_origin_type` instead.
pub fn captured_reference_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Option<StructuralTypeId> {
    let geometry = path
        .iter()
        .map(|projection| match projection {
            ReferenceOriginProjection::Exact(StructuralPathSegment::RuntimeIndex { .. }) => {
                ReferenceOriginProjection::Element
            }
            _ => projection.clone(),
        })
        .collect::<Vec<_>>();
    reference_origin_type(types, root, &geometry)
}

/// Resolve every declaration projection and collection dimension. This grants
/// neither bounds nor residence: those belong to the captured returned loan.
pub fn reference_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    mut root: StructuralTypeId,
    path: &[ReferenceOriginProjection],
) -> Option<StructuralTypeId> {
    let mut exact = Vec::new();
    for projection in path {
        match projection {
            ReferenceOriginProjection::Exact(StructuralPathSegment::FixedIndex(index)) => {
                root = exact_origin_type(types.clone(), root, &exact)?;
                exact.clear();
                let collection = types.clone().find(|declaration| declaration.id == root)?;
                if matches!(collection.shape, StructuralTypeShape::FixedArray { length, .. } if *index >= length)
                {
                    return None;
                }
                // An input view has no static length. This resolves its
                // element's type, not whether the captured index was in bounds.
                root = collection_element_type(types.clone(), root)?;
            }
            ReferenceOriginProjection::Exact(segment) => {
                if matches!(
                    segment,
                    StructuralPathSegment::RuntimeIndex { .. }
                        | StructuralPathSegment::FixedByteRange { .. }
                ) {
                    return None;
                }
                exact.push(segment.clone());
            }
            ReferenceOriginProjection::Element => {
                root = exact_origin_type(types.clone(), root, &exact)?;
                exact.clear();
                root = collection_element_type(types.clone(), root)?;
            }
        }
    }
    exact_origin_type(types, root, &exact)
}

fn exact_origin_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let path = crate::reference_source_storage_path(types.clone(), root, path);
    crate::runtime_structural_path_tip(types.clone(), root, path)
        .map(|declaration| declaration.id)
        .or_else(|| crate::primitive_projection_declaration(types, root, path))
}

/// Byte collections have an implicit u8 element declaration; ordinary arrays
/// and element views name theirs explicitly. No capacity enumeration is needed.
pub fn collection_element_type<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    collection: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types
        .clone()
        .find(|declaration| declaration.id == collection)?;
    match declaration.shape {
        StructuralTypeShape::FixedArray { element, .. }
        | StructuralTypeShape::ElementView { element } => Some(element),
        StructuralTypeShape::ByteSequence(_) => {
            let mut matching = types.filter(|declaration| declaration.scalar_restriction.is_none()
                && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if integer.sign() == IntegerSign::Unsigned && integer.bits() == 8 && !integer.is_address()));
            let element = matching.next()?;
            matching.next().is_none().then_some(element.id)
        }
        _ => None,
    }
}
