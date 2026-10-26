use super::*;
use terminal_psi::TerminalIdentityIndex;

#[test]
fn descriptor_extents_require_exact_readable_crossings() {
    let bytes = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: false,
        descriptor_cleanup: true,
        shape,
        scalar_restriction: None,
    };
    let mut types = vec![
        declaration(
            bytes,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: bytes,
                length: 1,
            },
        ),
        declaration(
            reference,
            StructuralTypeShape::Reference {
                referent: array,
                access: StructuralAccess::MutableBorrow,
            },
        ),
    ];
    let path = vec![
        ScalarReadProjection::Referent,
        ScalarReadProjection::Static(Static::FixedIndex(0)),
        ScalarReadProjection::Extent,
    ];
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path),
        Some(count)
    );
    // Inline shape cannot stand in for a missing pointer crossing, and a
    // fixed array has static type geometry rather than a live descriptor.
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path[1..]),
        None
    );
    assert_eq!(
        structural_read_scalar_type(types.iter(), array, &[ScalarReadProjection::Extent]),
        None
    );
    let mut outside = path.clone();
    outside[1] = ScalarReadProjection::Static(Static::FixedIndex(1));
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &outside),
        None
    );
    let mut continued = path.clone();
    continued.push(ScalarReadProjection::Extent);
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &continued),
        None
    );
    types[2].shape = StructuralTypeShape::Reference {
        referent: array,
        access: StructuralAccess::WriteOnlyBorrow,
    };
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path),
        None
    );
    types[2].shape = StructuralTypeShape::Reference {
        referent: array,
        access: StructuralAccess::MutableBorrow,
    };
    types[0].shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::WriteOnlyBorrow),
    });
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path),
        None
    );
}
