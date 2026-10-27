use super::*;
use semantic_vocabulary::{
    DomainSemanticId, PlaceId, Proposition, ScalarTerm, ScalarType, StructuralFieldId,
};
use terminal_psi::{
    BindingRelevance, NominalDeclarationIdentity, StructuralDomainPredicate,
    StructuralEstablishmentRoute, StructuralFieldDeclaration, StructuralRecordStorage,
    TerminalIdentityIndex,
};

#[test]
fn declared_array_leaf_proves_its_predicate_without_creating_extra_custody() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let bounds = BoundedIntegerType::new(
        integer,
        IntegerValue::Unsigned(0),
        IntegerValue::Unsigned(100),
    )
    .unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    let array = StructuralTypeId::new(3).unwrap();
    let field = TerminalIdentityIndex(2);
    let declaration = |id, shape, restriction| StructuralTypeDeclaration {
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        shape,
        scalar_restriction: restriction,
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: true,
    };
    let mut types = vec![
        declaration(
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
            Some(StructuralFieldType::BoundedInteger(bounds)),
        ),
        declaration(
            record,
            StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: field,
                    relevance: BindingRelevance::Relevant,
                    storage: None,
                    field_type: StructuralFieldType::BoundedInteger(bounds),
                }],
            },
            None,
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: record,
                length: 3,
            },
            None,
        ),
    ];
    let subject = PlaceId::new(1).unwrap();
    let predicate = |maximum| StructuralDomainPredicate {
        subject,
        proposition: Proposition::LessOrEqual(
            ScalarTerm::StructuralRead {
                root: subject,
                path: Vec::new(),
                scalar_type: ScalarType::Integer(integer),
            },
            ScalarTerm::Integer {
                scalar_type: integer,
                value: IntegerValue::Unsigned(maximum),
            },
        ),
    };
    let domain = StructuralDomainDeclaration {
        id: StructuralDomainId::new(1).unwrap(),
        identity: TerminalIdentityIndex(3),
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        carrier: scalar,
        predicate: Some(predicate(100)),
        complete_predicate_theory: true,
        required_memberships: Some(Vec::new()),
        establishment_routes: Vec::new(),
        content_projection: None,
        byte_predicate: None,
    };
    let path = [
        StructuralPathSegment::FixedIndex(1),
        StructuralPathSegment::Field(field),
    ];
    let entails = |types: &[StructuralTypeDeclaration], domain: &StructuralDomainDeclaration| {
        scalar_projection_entails_membership(
            types.iter(),
            std::iter::once(domain),
            array,
            &path,
            domain.id,
        )
    };
    assert!(entails(&types, &domain));
    // A current value of 73 would satisfy this stronger predicate, but the
    // source declaration admits 100. Formation must not invent that debt.
    let mut stronger = domain.clone();
    stronger.predicate = Some(predicate(73));
    assert!(!entails(&types, &stronger));
    let mut routed = domain.clone();
    routed
        .establishment_routes
        .push(StructuralEstablishmentRoute::ExactMachine {
            declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(4)),
            machine_identity: TerminalIdentityIndex(5),
        });
    assert!(!entails(&types, &routed));
    let mut incomplete = domain.clone();
    incomplete.complete_predicate_theory = false;
    assert!(!entails(&types, &incomplete));
    let mut unknown = domain.clone();
    unknown.required_memberships = None;
    assert!(!entails(&types, &unknown));
    let mut wrong_carrier = domain.clone();
    wrong_carrier.carrier = record;
    assert!(!entails(&types, &wrong_carrier));
    assert!(!scalar_projection_entails_membership(
        types.iter(),
        std::iter::once(&domain),
        array,
        &[
            StructuralPathSegment::FixedIndex(3),
            StructuralPathSegment::Field(field)
        ],
        domain.id
    ));
    // Equal-width unbounded or atomic storage cannot borrow validity from an
    // unrelated restricted scalar declaration elsewhere in the catalog.
    for field_type in [
        StructuralFieldType::Scalar(ScalarType::Integer(integer)),
        StructuralFieldType::AtomicBoundedInteger(bounds),
    ] {
        let StructuralTypeShape::Record { fields, .. } = &mut types[1].shape else {
            unreachable!()
        };
        fields[0].field_type = field_type;
        assert!(!entails(&types, &domain));
    }
}
