//! Projections ending in a primitive structural referent.
//!
//! A primitive leaf is a `PrimitiveScalar` place: a whole primitive root, an
//! element reached through record fields holding structural children and
//! fixed-array elements, or a byte carrier's u8 element. Primitive stores and reads name it by a
//! `StructuralPathSegment` projection whose elements may be runtime-selected
//! (`primitive_projection_type`); a static projection also has one canonical
//! spelling (`canonical_primitive_path`) that facts are keyed by. The backend's
//! abstract operations keep the canonical form (`primitive_place_type`).

use crate::static_path::canonical_structural_path_tip;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// Reconstruct the primitive leaf from declaration identities and fixed bounds.
/// This grants no access or establishment authority for the root.
pub fn primitive_place_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<ScalarType> {
    match canonical_structural_path_tip(declarations, root, path)?.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
        _ => None,
    }
}

/// Resolve `path` from `root` to a fixed array whose element declaration is a
/// primitive scalar, returning that scalar type and the declared extent. The
/// backend's abstract indexed read and store name the array this way and carry
/// the runtime element as an operand; Terminal spells the same element as the
/// path's trailing `RuntimeIndex` segment.
pub fn fixed_array_place_shape<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<(ScalarType, u64)> {
    let declaration = canonical_structural_path_tip(declarations.clone(), root, path)?;
    let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
        return None;
    };
    let mut matching = declarations.filter(|declaration| declaration.id == element);
    let element_declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match element_declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => Some((scalar, length)),
        _ => None,
    }
}

/// Resolve a primitive leaf projection that may select elements at run time.
/// Fields step only into structural children, a literal index stays inside
/// its extent, and a runtime index steps into any fixed array's element: its
/// bound is the obligation of the operation carrying the path. `Referent`
/// names no primitive leaf; a byte carrier's element is the u8 scalar.
/// This grants no access authority.
pub fn primitive_projection_type<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    walk_primitive_projection(declarations, root, path, |_| {})?.primitive_scalar()
}

/// The canonical spelling of a static primitive projection, the key facts
/// about the leaf are recorded under. `None` for a projection with a
/// runtime-selected element: it names no single place.
pub fn canonical_primitive_path<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let (canonical, tip) = canonical_static_projection(declarations, root, path)?;
    matches!(tip.shape, StructuralTypeShape::PrimitiveScalar(_)).then_some(canonical)
}

/// The canonical spelling of any static projection of fields holding
/// structural children and literal elements, with the declaration it
/// selects. `None` when a segment is runtime-selected or names no place.
pub fn canonical_static_projection<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<(
    Vec<CanonicalStructuralPathSegment>,
    &'a StructuralTypeDeclaration,
)> {
    let mut canonical = Some(Vec::with_capacity(path.len()));
    let ProjectionTip::Declared(tip) =
        walk_primitive_projection(declarations, root, path, |step| {
            match (step, &mut canonical) {
                (Some(segment), Some(canonical)) => canonical.push(segment),
                (None, canonical) => *canonical = None,
                (Some(_), None) => {}
            }
        })?
    else {
        return None;
    };
    Some((canonical?, tip))
}

/// The tip of a primitive projection: a declared structural type, or a byte
/// carrier's element — the u8 scalar that rides no declared identity.
enum ProjectionTip<'a> {
    Declared(&'a StructuralTypeDeclaration),
    ByteElement,
}

impl ProjectionTip<'_> {
    fn primitive_scalar(&self) -> Option<ScalarType> {
        match self {
            Self::Declared(declaration) => match declaration.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(scalar),
                _ => None,
            },
            Self::ByteElement => Some(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            )),
        }
    }
}

/// One walk for both entry points. `step` receives each segment's canonical
/// spelling, or `None` for a runtime-selected element.
fn walk_primitive_projection<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    mut step: impl FnMut(Option<CanonicalStructuralPathSegment>),
) -> Option<ProjectionTip<'a>> {
    enum Position {
        Declared(StructuralTypeId),
        ByteElement,
    }
    let unique = |id: StructuralTypeId| {
        let mut matching = declarations
            .clone()
            .filter(move |declaration| declaration.id == id);
        let declaration = matching.next()?;
        matching.next().is_none().then_some(declaration)
    };
    let mut position = Position::Declared(root);
    for segment in path {
        let Position::Declared(root) = position else {
            return None;
        };
        let declaration = unique(root)?;
        position = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let mut matching = fields.iter().filter(|field| field.identity == *identity);
                let field = matching.next()?;
                if matching.next().is_some() || field.relevance.is_erased() {
                    return None;
                }
                let StructuralFieldType::Structural(child) = field.field_type else {
                    return None;
                };
                step(Some(CanonicalStructuralPathSegment::Field(field.id)));
                Position::Declared(child)
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => {
                step(Some(CanonicalStructuralPathSegment::FixedIndex(*index)));
                Position::Declared(*element)
            }
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => {
                step(None);
                Position::Declared(*element)
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::ByteSequence(carrier),
            ) => {
                if let ByteSequenceCarrier::BoundedView { capacity, .. } = carrier
                    && index >= capacity
                {
                    return None;
                }
                step(Some(CanonicalStructuralPathSegment::FixedIndex(*index)));
                Position::ByteElement
            }
            (StructuralPathSegment::RuntimeIndex { .. }, StructuralTypeShape::ByteSequence(_)) => {
                step(None);
                Position::ByteElement
            }
            _ => return None,
        };
    }
    match position {
        Position::Declared(root) => unique(root).map(ProjectionTip::Declared),
        Position::ByteElement => Some(ProjectionTip::ByteElement),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CanonicalStructuralPathSegment, ScalarType, StructuralTypeDeclaration, StructuralTypeId,
        StructuralTypeShape, primitive_place_type,
    };
    use semantic_vocabulary::{IntegerSign, IntegerType, StructuralFieldId};
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};

    #[test]
    fn primitive_projection_checks_selected_fields_and_extents_not_unrelated_erased_metadata() {
        let primitive = StructuralTypeId::new(1).unwrap();
        let array = StructuralTypeId::new(2).unwrap();
        let record = StructuralTypeId::new(3).unwrap();
        let erased = StructuralFieldId::new(1).unwrap();
        let bytes = StructuralFieldId::new(2).unwrap();
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        let declarations = [
            StructuralTypeDeclaration {
                id: primitive,
                identity: "Octet".into(),
                shape: StructuralTypeShape::PrimitiveScalar(scalar),
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "Bytes".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: primitive,
                    length: 256,
                },
            },
            StructuralTypeDeclaration {
                id: record,
                identity: "Receiver".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![
                        StructuralFieldDeclaration {
                            id: erased,
                            identity: "console".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Erased {
                                type_identity: "Console".into(),
                            },
                        },
                        StructuralFieldDeclaration {
                            id: bytes,
                            identity: "bytes".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(array),
                        },
                    ],
                },
            },
        ];
        assert_eq!(
            primitive_place_type(declarations.iter(), primitive, &[]),
            Some(scalar)
        );
        let path = [
            CanonicalStructuralPathSegment::Field(bytes),
            CanonicalStructuralPathSegment::FixedIndex(255),
        ];
        assert_eq!(
            primitive_place_type(declarations.iter(), record, &path),
            Some(scalar)
        );
        for path in [
            vec![CanonicalStructuralPathSegment::Field(erased)],
            vec![CanonicalStructuralPathSegment::Field(bytes)],
            vec![
                CanonicalStructuralPathSegment::Field(bytes),
                CanonicalStructuralPathSegment::FixedIndex(256),
            ],
        ] {
            assert_eq!(
                primitive_place_type(declarations.iter(), record, &path),
                None
            );
        }
    }
}
