//! Address geometry preserves pointer boundaries; live loan authority is checked separately.

use super::{PointerSlot, RuntimeElement, primitive_store, shape};
use abstract_operations::StructuralScalarStoreDestination;
use semantic_vocabulary::{
    IntegerSign, IntegerType, ObligationId, PlaceId, ScalarType, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn identity(number: u64) -> StructuralTypeId {
    StructuralTypeId::new(number).unwrap()
}

fn scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn destination(
    root: StructuralTypeId,
    access: StructuralAccess,
) -> StructuralScalarStoreDestination {
    StructuralScalarStoreDestination::Parameter(StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access,
        qualifications: vec![],
        projected_qualifications: vec![],
    })
}

fn declarations() -> Vec<StructuralTypeDeclaration> {
    [
        StructuralTypeShape::PrimitiveScalar(scalar()),
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
                    field_type: StructuralFieldType::Scalar(scalar()),
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
fn primitive_store_bounds_the_pointer_slot_and_referent_separately() {
    let declarations = declarations();
    let path = [
        StructuralPathSegment::Field("destination".into()),
        StructuralPathSegment::Referent,
        StructuralPathSegment::FixedIndex(2),
    ];
    let geometry = primitive_store(
        &destination(identity(4), StructuralAccess::MutableBorrow),
        &path,
        scalar(),
        &declarations,
    )
    .expect("the referent leaf need not fit inside the carrier record");
    assert_eq!(shape(identity(4), &declarations).unwrap().byte_size, 16);
    assert_eq!(
        geometry.pointer_slots,
        [PointerSlot {
            byte_offset: 8,
            indices: 0..0,
        }]
    );
    assert_eq!((geometry.byte_offset, geometry.byte_size), (16, 8));
    assert!(geometry.elements.is_empty());
}

#[test]
fn primitive_store_partitions_runtime_indices_around_the_pointer_load() {
    let declarations = declarations();
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
    let geometry = primitive_store(
        &destination(identity(5), StructuralAccess::MutableBorrow),
        &path,
        scalar(),
        &declarations,
    )
    .expect("both regions retain their own element coordinates");
    assert_eq!(
        geometry.pointer_slots,
        [PointerSlot {
            byte_offset: 8,
            indices: 0..1,
        }]
    );
    assert_eq!(
        geometry.elements,
        [
            RuntimeElement {
                index: ValueId::new(10).unwrap(),
                obligation: ObligationId::new(20).unwrap(),
                stride: 16,
                extent: Some(3),
            },
            RuntimeElement {
                index: ValueId::new(11).unwrap(),
                obligation: ObligationId::new(21).unwrap(),
                stride: 8,
                extent: Some(4),
            },
        ]
    );
    let leaf_indices = geometry.pointer_slots.last().unwrap().indices.end..geometry.elements.len();
    assert_eq!(leaf_indices, 1..2);
    assert_eq!((geometry.byte_offset, geometry.byte_size), (0, 8));
}

#[test]
fn primitive_store_rejects_non_reference_crossing_bad_bound_and_shared_root() {
    let declarations = declarations();
    assert!(
        primitive_store(
            &destination(identity(1), StructuralAccess::MutableBorrow),
            &[StructuralPathSegment::Referent],
            scalar(),
            &declarations,
        )
        .is_none()
    );
    assert!(
        primitive_store(
            &destination(identity(3), StructuralAccess::MutableBorrow),
            &[
                StructuralPathSegment::Referent,
                StructuralPathSegment::FixedIndex(4),
            ],
            scalar(),
            &declarations,
        )
        .is_none()
    );
    assert!(
        primitive_store(
            &destination(identity(4), StructuralAccess::SharedBorrow),
            &[
                StructuralPathSegment::Field("destination".into()),
                StructuralPathSegment::Referent,
                StructuralPathSegment::FixedIndex(2),
            ],
            scalar(),
            &declarations,
        )
        .is_none()
    );
}

#[test]
fn primitive_store_retains_empty_regions_between_repeated_crossings() {
    let mut declarations = declarations();
    for (number, referent) in [(6, identity(1)), (7, identity(6))] {
        declarations.push(StructuralTypeDeclaration {
            id: identity(number),
            identity: format!("reference_{number}"),
            shape: StructuralTypeShape::Reference {
                referent,
                access: StructuralAccess::MutableBorrow,
            },
        });
    }
    let geometry = primitive_store(
        &destination(identity(7), StructuralAccess::MutableBorrow),
        &[
            StructuralPathSegment::Referent,
            StructuralPathSegment::Referent,
        ],
        scalar(),
        &declarations,
    )
    .expect("each empty region still ends in its own pointer load");
    assert_eq!(
        geometry.pointer_slots,
        [
            PointerSlot {
                byte_offset: 0,
                indices: 0..0
            },
            PointerSlot {
                byte_offset: 0,
                indices: 0..0
            },
        ]
    );
    assert_eq!((geometry.byte_offset, geometry.byte_size), (0, 8));
    assert!(geometry.elements.is_empty());
}

#[test]
fn primitive_store_keeps_ordinary_contiguous_projection() {
    let declarations = declarations();
    let geometry = primitive_store(
        &destination(identity(2), StructuralAccess::WriteOnlyBorrow),
        &[StructuralPathSegment::FixedIndex(2)],
        scalar(),
        &declarations,
    )
    .expect("ordinary array store needs no pointer load");
    assert!(geometry.pointer_slots.is_empty());
    assert!(geometry.elements.is_empty());
    assert_eq!((geometry.byte_offset, geometry.byte_size), (16, 8));
}
