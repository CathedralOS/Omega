//! Empty ABI shapes retain complete recursive primitive declarations.

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
fn empty_primitive_array_reconstruction_rejects_unknown_recursive_and_nonprimitive_types() {
    for nested in [
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(99).unwrap(),
            length: 0,
        },
        StructuralTypeShape::FixedArray {
            element: StructuralTypeId::new(1).unwrap(),
            length: 0,
        },
        StructuralTypeShape::Record { fields: Vec::new() },
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
