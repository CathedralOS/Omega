use super::*;
use semantic_vocabulary::{BoundedIntegerType, IntegerSign, IntegerType, IntegerValue};

#[test]
fn bounded_copy_preserves_exact_restrictions_and_plain_observations() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let field = StructuralFieldType::BoundedInteger(
        BoundedIntegerType::new(
            integer,
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(8),
        )
        .unwrap(),
    );
    let plain = StructuralTypeDeclaration {
        quotient_carrier: None,
        id: StructuralTypeId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        copyable: true,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
        scalar_restriction: None,
    };
    assert_eq!(
        field.leaf_copy_type(Some(&plain)),
        Some(StructuralFieldType::Structural(plain.id))
    );
    assert_eq!(field.leaf_copy_type(None), Some(field.clone()));
    let exact = StructuralTypeDeclaration {
        id: StructuralTypeId::new(2).unwrap(),
        scalar_restriction: Some(field.clone()),
        ..plain.clone()
    };
    assert_eq!(
        field.leaf_copy_type(Some(&exact)),
        Some(StructuralFieldType::Structural(exact.id))
    );
    let narrower = StructuralTypeDeclaration {
        id: StructuralTypeId::new(3).unwrap(),
        scalar_restriction: Some(StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                integer,
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(4),
            )
            .unwrap(),
        )),
        ..plain
    };
    assert!(
        !field
            .leaf_copy_type(Some(&narrower))
            .unwrap()
            .declares_leaf(&narrower)
    );
}
