use super::{canonical_field_storage_region, projected_storage_region};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, ObligationId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn declarations() -> Vec<StructuralTypeDeclaration> {
    vec![
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Item".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(7).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "Items".into(),
            shape: StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(1).unwrap(),
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(3).unwrap(),
            identity: "Container".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(9).unwrap(),
                    identity: "items".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(StructuralTypeId::new(2).unwrap()),
                }],
            },
        },
    ]
}

fn runtime_index() -> StructuralPathSegment {
    StructuralPathSegment::RuntimeIndex {
        index: ValueId::new(1).unwrap(),
        obligation: ObligationId::new(1).unwrap(),
    }
}

#[test]
fn runtime_selector_preserves_enclosing_storage_and_later_indirection() {
    let declarations = declarations();
    let region = projected_storage_region(
        declarations.iter(),
        StructuralTypeId::new(3).unwrap(),
        &[
            StructuralPathSegment::Field("items".into()),
            runtime_index(),
            StructuralPathSegment::Referent,
            StructuralPathSegment::Field("value".into()),
        ],
        None,
    );
    assert_eq!(
        region.path,
        vec![StructuralPathSegment::Field("items".into())]
    );
    assert!(
        region.indirect,
        "truncating geometry must not erase indirection"
    );
    assert!(
        region.direct_path().is_empty(),
        "ordinary callers cannot derive direct alias freedom from an indirect region"
    );
    assert!(crate::structural_paths_may_overlap(
        &region.path,
        &[
            StructuralPathSegment::Field("items".into()),
            StructuralPathSegment::FixedIndex(0),
        ],
    ));
}

#[test]
fn reference_crossing_is_metadata_not_a_synthetic_storage_segment() {
    let declarations = declarations();
    let region = projected_storage_region(
        declarations.iter(),
        StructuralTypeId::new(3).unwrap(),
        &[
            StructuralPathSegment::Referent,
            StructuralPathSegment::Field("items".into()),
        ],
        None,
    );
    assert!(region.path.is_empty());
    assert!(region.indirect);
}

#[test]
fn runtime_selector_without_reference_stays_direct_and_covers_all_elements() {
    let declarations = declarations();
    let region = projected_storage_region(
        declarations.iter(),
        StructuralTypeId::new(3).unwrap(),
        &[
            StructuralPathSegment::Field("items".into()),
            runtime_index(),
        ],
        Some(StructuralFieldId::new(7).unwrap()),
    );
    assert_eq!(
        region.path,
        vec![StructuralPathSegment::Field("items".into())]
    );
    assert!(!region.indirect);
    assert_eq!(region.direct_path(), region.path);
}

#[test]
fn static_final_field_rejoins_declaration_identity_and_canonical_projection() {
    let declarations = declarations();
    let root = StructuralTypeId::new(3).unwrap();
    let field = StructuralFieldId::new(7).unwrap();
    let region = projected_storage_region(
        declarations.iter(),
        root,
        &[
            StructuralPathSegment::Field("items".into()),
            StructuralPathSegment::FixedIndex(1),
        ],
        Some(field),
    );
    assert_eq!(
        region.path,
        vec![
            StructuralPathSegment::Field("items".into()),
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Field("value".into()),
        ]
    );
    assert!(!region.indirect);
    assert_eq!(
        canonical_field_storage_region(
            declarations.iter(),
            root,
            &[
                CanonicalStructuralPathSegment::Field(StructuralFieldId::new(9).unwrap()),
                CanonicalStructuralPathSegment::FixedIndex(1)
            ],
            field,
        ),
        region.path
    );
}

#[test]
fn unresolved_final_field_preserves_parent_region_without_inventing_a_sibling() {
    let declarations = declarations();
    let root = StructuralTypeId::new(3).unwrap();
    let unknown = StructuralFieldId::new(99).unwrap();
    let parent = vec![
        StructuralPathSegment::Field("items".into()),
        StructuralPathSegment::FixedIndex(1),
    ];
    let region = projected_storage_region(declarations.iter(), root, &parent, Some(unknown));
    assert_eq!(region.path, parent);
    assert!(!region.indirect);
    assert!(
        canonical_field_storage_region(
            declarations.iter(),
            root,
            &[
                CanonicalStructuralPathSegment::Field(StructuralFieldId::new(9).unwrap()),
                CanonicalStructuralPathSegment::FixedIndex(1)
            ],
            unknown,
        )
        .is_empty(),
        "unresolved canonical geometry covers the whole root"
    );
}
