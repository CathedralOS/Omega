use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, StructuralTypeId};
use terminal_psi::{StructuralFieldType, TerminalIdentityIndex};

fn integer(sign: IntegerSign, bits: u16) -> ScalarType {
    ScalarType::Integer(IntegerType::new(sign, bits).expect("fixed integer"))
}

fn primitive(raw: u64, scalar: ScalarType) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        id: StructuralTypeId::new(raw).expect("type identity"),
        identity: TerminalIdentityIndex(raw as u32),
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        quotient_carrier: None,
        scalar_restriction: None,
        shape: StructuralTypeShape::PrimitiveScalar(scalar),
    }
}

// `&mut pair.start as &mut u32` over an i32 field: every bit pattern is
// valid in both carriers, so every borrow access admits the view, but an
// owned value is never reinterpreted in place.
#[test]
fn equal_width_integer_views_admit_every_borrow() {
    let signed = integer(IntegerSign::Signed, 32);
    let unsigned = integer(IntegerSign::Unsigned, 32);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        assert!(scalar_reference_view_admits(access, signed, unsigned));
        assert!(scalar_reference_view_admits(access, unsigned, signed));
    }
    assert!(!scalar_reference_view_admits(
        StructuralAccess::Owned,
        signed,
        unsigned
    ));
}

// Every bool reads as a valid u8, but a u8 write of 2 would leave invalid
// bool storage: only the direction the view's operations need is admitted.
#[test]
fn one_way_views_admit_only_the_operations_they_keep_valid() {
    let byte = integer(IntegerSign::Unsigned, 8);
    let boolean = ScalarType::Boolean;
    assert!(scalar_reference_view_admits(
        StructuralAccess::SharedBorrow,
        boolean,
        byte
    ));
    assert!(!scalar_reference_view_admits(
        StructuralAccess::MutableBorrow,
        boolean,
        byte
    ));
    assert!(!scalar_reference_view_admits(
        StructuralAccess::WriteOnlyBorrow,
        boolean,
        byte
    ));
    assert!(scalar_reference_view_admits(
        StructuralAccess::WriteOnlyBorrow,
        byte,
        boolean
    ));
    assert!(!scalar_reference_view_admits(
        StructuralAccess::SharedBorrow,
        byte,
        boolean
    ));
}

#[test]
fn borrowed_fields_present_only_admitted_scalar_views() {
    let signed = StructuralFieldType::Scalar(integer(IntegerSign::Signed, 32));
    let unsigned = primitive(7, integer(IntegerSign::Unsigned, 32));
    assert_eq!(
        borrowed_field_presentation(
            &signed,
            StructuralAccess::MutableBorrow,
            Some(&unsigned)
        ),
        Some(StructuralFieldType::Structural(unsigned.id))
    );
    let boolean = StructuralFieldType::Scalar(ScalarType::Boolean);
    let byte = primitive(8, integer(IntegerSign::Unsigned, 8));
    assert_eq!(
        borrowed_field_presentation(&boolean, StructuralAccess::SharedBorrow, Some(&byte)),
        Some(StructuralFieldType::Structural(byte.id))
    );
    assert_eq!(
        borrowed_field_presentation(&boolean, StructuralAccess::MutableBorrow, Some(&byte)),
        Some(boolean)
    );
}
