//! Empty ABI shapes retain complete recursive element declarations.

use super::{
    IeeeFloatFormat, ScalarType, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueShape,
    project_static_path, reconstruct,
};
fn declaration(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: format!("test::shape_{id}"),
        shape,
    }
}

#[test]
fn static_projection_replays_interleaved_array_stride_and_record_padding() {
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration};

    let types = [
        declaration(
            1,
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(2).unwrap(),
                length: 2,
            },
        ),
        declaration(
            2,
            StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "prefix".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "items".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(
                            StructuralTypeId::new(3).unwrap(),
                        ),
                    },
                ],
            },
        ),
        declaration(
            3,
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(4).unwrap(),
                length: 3,
            },
        ),
        declaration(
            4,
            StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        ),
    ];
    let path = [
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field("items".into()),
        StructuralPathSegment::FixedIndex(2),
    ];
    assert_eq!(
        project_static_path(types[0].id, &path, &types),
        Ok((types[3].id, 28))
    );
    for (position, segment) in [
        (0, StructuralPathSegment::FixedIndex(2)),
        (1, StructuralPathSegment::Field("missing".into())),
        (1, StructuralPathSegment::Referent),
        (2, StructuralPathSegment::FixedIndex(3)),
    ] {
        let mut invalid = path.clone();
        invalid[position] = segment;
        assert!(project_static_path(types[0].id, &invalid, &types).is_err());
    }
}

#[test]
fn empty_primitive_arrays_have_canonical_zero_byte_abi_without_inner_extent_overflow() {
    for lengths in [[0, u64::MAX], [u64::MAX, 0]] {
        let types = [
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(2).unwrap(),
                    length: lengths[0],
                },
            ),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(3).unwrap(),
                    length: lengths[1],
                },
            ),
            declaration(
                3,
                StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                    IeeeFloatFormat::Binary64,
                )),
            ),
        ];
        assert_eq!(
            reconstruct(types[0].id, &types),
            Ok(ValueShape::integer(0, 1))
        );
    }
}

#[test]
fn empty_array_reconstruction_rejects_unknown_and_recursive_types() {
    for nested in [
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(99).unwrap(),
            length: 0,
        },
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(1).unwrap(),
            length: 0,
        },
    ] {
        let types = [
            declaration(
                1,
                StructuralTypeShape::FixedArray {
                    element: StructuralTypeId::new(2).unwrap(),
                    length: 0,
                },
            ),
            declaration(2, nested),
        ];
        assert!(reconstruct(types[0].id, &types).is_err());
    }
}

#[test]
fn empty_record_array_geometry_keeps_element_validation_and_no_index() {
    let types = [
        declaration(
            1,
            StructuralTypeShape::FixedArray {
                element: StructuralTypeId::new(2).unwrap(),
                length: 0,
            },
        ),
        declaration(2, StructuralTypeShape::Record { fields: Vec::new() }),
    ];
    assert_eq!(
        reconstruct(types[0].id, &types),
        Ok(ValueShape::integer(0, 1))
    );
    assert!(
        project_static_path(types[0].id, &[StructuralPathSegment::FixedIndex(0)], &types).is_err()
    );
}

#[test]
fn case_payload_projection_retains_prefix_stride_and_exact_case_field() {
    use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
    use terminal_psi::{BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration};
    let leaf = StructuralTypeId::new(3).unwrap();
    let sum = StructuralTypeId::new(2).unwrap();
    let root = StructuralTypeId::new(1).unwrap();
    let selected_case = StructuralCaseId::new(21).unwrap();
    let mut types = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "Slots".into(),
            shape: StructuralTypeShape::FixedArray {
                element: sum,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: sum,
            identity: "Choice".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: selected_case,
                        identity: "Present".into(),
                        fields: vec![StructuralFieldDeclaration {
                            id: StructuralFieldId::new(31).unwrap(),
                            identity: "item".into(),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Structural(leaf),
                        }],
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(22).unwrap(),
                        identity: "Absent".into(),
                        fields: Vec::new(),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: leaf,
            identity: "Item".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(32).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    )),
                }],
            },
        },
    ];
    // Each sum is 16 bytes: tag at zero, eight-byte payload at eight.
    // Borrowing the second element's payload retains backing offset 24.
    let path = vec![
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Case(selected_case),
        StructuralPathSegment::Field("item".into()),
    ];
    assert_eq!(project_static_path(root, &path, &types), Ok((leaf, 24)));
    for invalid in [
        vec![
            StructuralPathSegment::FixedIndex(2),
            StructuralPathSegment::Case(selected_case),
            StructuralPathSegment::Field("item".into()),
        ],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(StructuralCaseId::new(22).unwrap()),
            StructuralPathSegment::Field("item".into()),
        ],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(selected_case),
        ],
        vec![
            StructuralPathSegment::FixedIndex(1),
            StructuralPathSegment::Case(selected_case),
            StructuralPathSegment::Field("missing".into()),
        ],
        vec![
            StructuralPathSegment::Case(selected_case),
            StructuralPathSegment::Field("item".into()),
        ],
    ] {
        assert!(project_static_path(root, &invalid, &types).is_err());
    }
    let StructuralTypeShape::Sum { cases } = &mut types[1].shape else {
        unreachable!()
    };
    cases[0].fields[0].relevance = BindingRelevance::Erased;
    assert!(project_static_path(root, &path, &types).is_err());
}
