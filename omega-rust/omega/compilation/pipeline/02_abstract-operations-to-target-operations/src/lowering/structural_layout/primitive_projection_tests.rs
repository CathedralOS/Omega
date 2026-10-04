use super::{StorageProjection, primitive_leaf_projection, primitive_read_projection};
use semantic_vocabulary::{IntegerSign, IntegerType, ObligationId, StructuralFieldId, ValueId};
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

fn identity(number: u64) -> StructuralTypeId {
    StructuralTypeId::new(number).unwrap()
}

fn declarations() -> Vec<StructuralTypeDeclaration> {
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    [
        StructuralTypeShape::PrimitiveScalar(scalar),
        StructuralTypeShape::FixedArray {
            element: identity(1),
            length: 4,
        },
        StructuralTypeShape::Reference {
            referent: identity(2),
            access: StructuralAccess::MutableBorrow,
        },
        StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "prefix".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(scalar),
                },
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: "destination".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(identity(3)),
                },
            ],
        },
        StructuralTypeShape::FixedArray {
            element: identity(4),
            length: 3,
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(position, shape)| StructuralTypeDeclaration {
        id: identity(position as u64 + 1),
        identity: format!("projection_type_{position}"),
        shape,
    })
    .collect()
}

#[test]
fn primitive_projection_crosses_nonzero_reference_slot_before_suffix() {
    let declarations = declarations();
    let lookup = declarations.iter().map(|row| (row.id, row)).collect();
    let path = [
        StructuralPathSegment::Field("destination".into()),
        StructuralPathSegment::Referent,
        StructuralPathSegment::FixedIndex(2),
    ];
    let projection = primitive_leaf_projection(identity(4), &path, &lookup).unwrap();
    assert_eq!(
        projection.pointer_slots,
        [StorageProjection {
            byte_offset: 8,
            indices: vec![]
        }]
    );
    assert_eq!(
        projection.leaf,
        StorageProjection {
            byte_offset: 16,
            indices: vec![]
        }
    );
    assert!(primitive_read_projection(identity(4), &path, &lookup).is_none());
}

#[test]
fn primitive_projection_retains_runtime_indices_on_both_sides_of_pointer_load() {
    let declarations = declarations();
    let lookup = declarations.iter().map(|row| (row.id, row)).collect();
    let path = [
        StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(10).unwrap(),
            obligation: ObligationId::new(20).unwrap(),
        },
        StructuralPathSegment::Field("destination".into()),
        StructuralPathSegment::Referent,
        StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(11).unwrap(),
            obligation: ObligationId::new(21).unwrap(),
        },
    ];
    let projection = primitive_leaf_projection(identity(5), &path, &lookup).unwrap();
    assert_eq!(
        projection.pointer_slots,
        [StorageProjection {
            byte_offset: 8,
            indices: vec![(ValueId::new(10).unwrap(), 16)],
        }]
    );
    assert_eq!(
        projection.leaf,
        StorageProjection {
            byte_offset: 0,
            indices: vec![(ValueId::new(11).unwrap(), 8)],
        }
    );
}

#[test]
fn primitive_projection_rejects_crossing_non_reference_and_invalid_referent_index() {
    let declarations = declarations();
    let lookup = declarations.iter().map(|row| (row.id, row)).collect();
    assert!(
        primitive_leaf_projection(identity(1), &[StructuralPathSegment::Referent], &lookup)
            .is_none()
    );
    assert!(
        primitive_leaf_projection(
            identity(3),
            &[
                StructuralPathSegment::Referent,
                StructuralPathSegment::FixedIndex(4),
            ],
            &lookup
        )
        .is_none()
    );
}

#[test]
fn primitive_projection_retains_empty_regions_between_repeated_crossings() {
    let mut declarations = declarations();
    declarations.extend([
        StructuralTypeDeclaration {
            id: identity(6),
            identity: "scalar_reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: identity(1),
                access: StructuralAccess::MutableBorrow,
            },
        },
        StructuralTypeDeclaration {
            id: identity(7),
            identity: "reference_reference".into(),
            shape: StructuralTypeShape::Reference {
                referent: identity(6),
                access: StructuralAccess::MutableBorrow,
            },
        },
    ]);
    let lookup = declarations.iter().map(|row| (row.id, row)).collect();
    let projection = primitive_leaf_projection(
        identity(7),
        &[
            StructuralPathSegment::Referent,
            StructuralPathSegment::Referent,
        ],
        &lookup,
    )
    .unwrap();
    assert_eq!(projection.pointer_slots.len(), 2);
    assert!(
        projection
            .pointer_slots
            .iter()
            .all(|slot| slot.byte_offset == 0 && slot.indices.is_empty())
    );
    assert_eq!(
        projection.leaf,
        StorageProjection {
            byte_offset: 0,
            indices: vec![]
        }
    );
}
