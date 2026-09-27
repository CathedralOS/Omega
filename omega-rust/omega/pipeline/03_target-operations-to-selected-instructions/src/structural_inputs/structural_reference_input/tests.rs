//! Structural reference input tests.

use super::{
    ScalarType, StructuralFieldId, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueShape,
};
use crate::structural_inputs::structural_reference_input::byte_field_length;
use crate::structural_inputs::structural_reference_input::field_read;
use crate::structural_inputs::structural_reference_input::field_shape;
use crate::structural_inputs::structural_reference_input::owned_aggregate_shape;
use crate::structural_inputs::structural_reference_input::plain_record_shape;
use crate::structural_inputs::structural_reference_input::shape;
use crate::structural_inputs::structural_reference_input::store;

#[test]
fn scalar_geometry_preserves_bounded_byte_siblings_without_owning_them() {
    use terminal_psi::{BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration};

    let root = StructuralTypeId::new(1).unwrap();
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap(),
    );
    let byte_field = StructuralFieldId::new(1).unwrap();
    let scalar_field = StructuralFieldId::new(2).unwrap();
    for (capacity, expected) in [
        (0, Some((16, 8))),
        (3, Some((16, 12))),
        (9, Some((24, 20))),
        (65_520, Some((65_536, 65_528))),
        (65_528, None),
        (u64::MAX, None),
    ] {
        let declarations = [StructuralTypeDeclaration {
            id: root,
            identity: "BufferedCounter".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: byte_field,
                        identity: "bytes".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::ByteSequence(
                            ByteSequenceCarrier::BoundedOwned { capacity },
                        ),
                    },
                    StructuralFieldDeclaration {
                        id: scalar_field,
                        identity: "counter".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    },
                ],
            },
        }];
        let expected = expected.filter(|(size, _)| *size <= u32::from(u16::MAX));
        assert_eq!(
            shape(root, &declarations),
            expected.map(|(size, _)| ValueShape::integer(size as u16, 8)),
        );
        let expected_access = expected.map(|(_, offset)| (offset, 4));
        assert_eq!(
            store(root, &[], scalar_field, scalar, &declarations),
            expected_access.map(|(offset, bytes)| (offset, bytes, Vec::new()))
        );
        assert_eq!(
            field_read(root, &[], scalar_field, scalar, &declarations),
            expected_access
        );
        assert_eq!(store(root, &[], byte_field, scalar, &declarations), None);
        let length_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .unwrap(),
        );
        assert_eq!(
            byte_field_length(root, &[], byte_field, length_type, &declarations),
            expected.map(|_| (0, 8)),
        );
        assert_eq!(
            byte_field_length(root, &[], byte_field, scalar, &declarations),
            None
        );
        assert_eq!(
            byte_field_length(root, &[], scalar_field, length_type, &declarations),
            None
        );
        for unrelated_carrier in [
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
            StructuralFieldType::Scalar(length_type),
        ] {
            let mut changed = declarations.clone();
            let StructuralTypeShape::Record { fields } = &mut changed[0].shape else {
                panic!("record");
            };
            fields[0].field_type = unrelated_carrier;
            assert_eq!(
                byte_field_length(root, &[], byte_field, length_type, &changed),
                None
            );
        }
        assert_eq!(
            field_read(root, &[], byte_field, scalar, &declarations),
            None
        );
        assert_eq!(
            field_read(root, &[], scalar_field, ScalarType::Boolean, &declarations),
            None,
        );
        assert_eq!(
            field_read(
                root,
                &[StructuralPathSegment::Field("bytes".into())],
                scalar_field,
                scalar,
                &declarations,
            ),
            None,
        );
        assert_eq!(owned_aggregate_shape(root, &declarations), None);
    }
}

#[test]
fn byte_field_layout_does_not_admit_owned_byte_roots() {
    use terminal_psi::ByteSequenceCarrier;

    assert_eq!(
        field_shape(
            &StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
            &[],
            &mut Vec::new(),
        ),
        Some(ValueShape::integer(16, 8)),
    );
    let root = StructuralTypeId::new(1).unwrap();
    let declarations = [StructuralTypeDeclaration {
        id: root,
        identity: "Buffer".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 3 }),
    }];
    assert_eq!(shape(root, &declarations), None);
    assert_eq!(plain_record_shape(root, &declarations), None);
    assert_eq!(owned_aggregate_shape(root, &declarations), None);
}

#[test]
fn mixed_declarations_have_tag_prefixed_geometry_without_field_access() {
    use terminal_psi::{BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration};

    let root = StructuralTypeId::new(1).unwrap();
    let mixed = StructuralTypeId::new(2).unwrap();
    let nested = StructuralTypeId::new(3).unwrap();
    let scalar = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .unwrap(),
    );
    let field = |id, identity: &str, field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let case = |id, identity: &str, fields| StructuralCaseDeclaration {
        id: semantic_vocabulary::StructuralCaseId::new(id).unwrap(),
        identity: identity.into(),
        fields,
    };
    // Tag at 0, common `flag` at 4, payloads overlaid at 8: `Raised` covers
    // twelve bytes at alignment four, identical to the target-side
    // `conventional_sum_layout_from_parts` evaluation of the same shape.
    let declarations = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "Main".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    field(1, "mixed", StructuralFieldType::Structural(mixed)),
                    field(2, "tail", StructuralFieldType::Scalar(scalar)),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: mixed,
            identity: "Mixed".into(),
            shape: StructuralTypeShape::Mixed {
                fields: vec![field(1, "flag", StructuralFieldType::Scalar(scalar))],
                cases: vec![
                    case(1, "Flat", vec![]),
                    case(
                        2,
                        "Raised",
                        vec![field(1, "level", StructuralFieldType::Scalar(scalar))],
                    ),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: nested,
            identity: "Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(1, "first", StructuralFieldType::Scalar(scalar))],
            },
        },
    ];
    assert_eq!(
        shape(mixed, &declarations),
        Some(ValueShape::integer(12, 4))
    );
    assert_eq!(shape(root, &declarations), Some(ValueShape::integer(16, 4)));
    // Structural payloads and erased-relevance common fields shape the same
    // way the target evaluator sees them; field paths into the mixed node
    // remain closed to scalar reads and stores.
    let mut with_structural = declarations.clone();
    let StructuralTypeShape::Mixed { cases, fields } = &mut with_structural[1].shape else {
        panic!("mixed");
    };
    cases[1].fields[0].field_type = StructuralFieldType::Structural(nested);
    fields.push(StructuralFieldDeclaration {
        relevance: BindingRelevance::Erased,
        ..field(2, "erased", StructuralFieldType::Scalar(scalar))
    });
    assert_eq!(
        shape(mixed, &with_structural),
        Some(ValueShape::integer(12, 4))
    );
    let flag = StructuralFieldId::new(1).unwrap();
    for path in [vec![], vec![StructuralPathSegment::Field("mixed".into())]] {
        assert_eq!(field_read(root, &path, flag, scalar, &declarations), None);
        assert_eq!(store(root, &path, flag, scalar, &declarations), None);
    }
    // An empty case set has no inhabitant to provision.
    let mut empty = declarations.clone();
    let StructuralTypeShape::Mixed { cases, .. } = &mut empty[1].shape else {
        panic!("mixed");
    };
    cases.clear();
    assert_eq!(shape(mixed, &empty), None);
    assert_eq!(shape(root, &empty), None);
}
