//! Empty ABI shapes retain complete recursive primitive declarations.
use super::*;

fn declaration(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: StructuralTypeId::new(id).unwrap(),
        identity: format!("test::shape_{id}"),
        shape,
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
