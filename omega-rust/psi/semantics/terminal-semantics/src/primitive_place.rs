//! Exact static projections ending in a primitive structural referent.

use crate::static_path::canonical_structural_path_tip;
use semantic_vocabulary::{CanonicalStructuralPathSegment, ScalarType, StructuralTypeId};
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

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
/// path tip is the array itself: the runtime element index is an operation
/// operand, not a path segment, so it never appears here.
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
