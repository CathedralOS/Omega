use super::*;

#[test]
fn empty_fixed_array_contains_no_reference() {
    let referent = StructuralTypeId::new(1).unwrap();
    let reference = StructuralTypeId::new(2).unwrap();
    let element = StructuralTypeId::new(3).unwrap();
    let array = StructuralTypeId::new(4).unwrap();
    let types = vec![
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: crate::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: reference,
            identity: crate::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Reference {
                referent,
                access: crate::StructuralAccess::Owned,
            },
        },
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: crate::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: referent,
            identity: crate::TerminalIdentityIndex(2),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        },
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: crate::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: element,
            identity: crate::TerminalIdentityIndex(3),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    declaration_identity: Default::default(),
                    inline_carrier: None,
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: crate::TerminalIdentityIndex(4),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(reference),
                    storage: None,
                }],
                storage: crate::StructuralRecordStorage::Packed,
            },
        },
        StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            multiplicity: crate::StructuralMultiplicity::Affine,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            scalar_restriction: None,
            id: array,
            identity: crate::TerminalIdentityIndex(5),
            shape: StructuralTypeShape::FixedArray {
                element,
                length: crate::ArrayExtent::Literal(0),
            },
        },
    ];
    assert!(structural_type_contains_reference(&types, reference));
    assert!(!structural_type_contains_reference(&types, array));
}
