use super::*;
use semantic_vocabulary::{ScalarType, StructuralTypeId};
use terminal_psi::{ArrayExtent, TerminalIdentityIndex};

#[test]
fn valid_utf8_does_not_impose_an_ascii_byte_store_bound() {
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let declaration = |id: StructuralTypeId, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
        requires: Vec::new(),
    };
    let types = [
        declaration(
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(byte)),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: scalar,
                length: ArrayExtent::Literal(2),
            },
        ),
    ];
    let identity = StructuralDomainId::new(1).unwrap();
    let restriction = |predicate| {
        let domain = StructuralDomainDeclaration {
            subject_identity: Default::default(),
            access_policy: None,
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: None,
            id: identity,
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(3),
            carrier: array,
            content_projection: None,
            byte_predicate: Some(predicate),
            establishment_routes: None,
        };
        primitive_store_restriction(
            types.iter(),
            std::iter::once(&domain),
            array,
            &[identity],
            &[],
            &[],
            &[StructuralPathSegment::FixedIndex(0)],
        )
        .expect("the byte predicate has an exact array and element path")
    };
    let utf8 = [0xC3, 0xA9];
    assert!(ByteSequencePredicate::ValidUtf8.holds_for(&utf8));
    assert!(!ByteSequencePredicate::AsciiOnly.holds_for(&utf8));
    assert_eq!(
        restriction(ByteSequencePredicate::ValidUtf8),
        None,
        "UTF-8 alone adds no individual-octet bound to u8"
    );
    let Some(StructuralFieldType::BoundedInteger(ascii)) =
        restriction(ByteSequencePredicate::AsciiOnly)
    else {
        panic!("ASCII must retain its individual-octet bound");
    };
    assert!(ascii.contains(IntegerValue::Unsigned(127)));
    for value in utf8 {
        assert!(byte.admits(IntegerValue::Unsigned(u128::from(value))));
        assert!(!ascii.contains(IntegerValue::Unsigned(u128::from(value))));
    }
}
