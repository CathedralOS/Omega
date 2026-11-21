use super::*;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, StructuralCaseId, StructuralCaseSubject,
    StructuralFieldId,
};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralTypeShape, TerminalIdentityIndex,
};

#[test]
fn extent_predicates_keep_the_exact_subject_carrier_and_storage_dependency() {
    let carrier = StructuralTypeId::new(1).unwrap();
    let subject = PlaceId::new(1).unwrap();
    let count = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let mut types = [StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id: carrier,
        identity: TerminalIdentityIndex(1),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape: StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            },
        ),
        scalar_restriction: None,
    }];
    let predicate = |root, path, scalar_type| StructuralDomainPredicate {
        subject,
        proposition: Proposition::Equal(
            ScalarTerm::ViewExtent {
                root,
                path,
                scalar_type,
            },
            ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(4)).unwrap(),
        ),
    };
    let valid = predicate(subject, Vec::new(), count);
    assert_eq!(
        predicate_dependencies(types.iter(), carrier, &valid),
        Some(vec![Vec::new()])
    );
    assert!(!domain_predicate_observes_no_storage(
        types.iter(),
        carrier,
        &valid
    ));
    assert!(!domain_predicate_is_closed(
        types.iter(),
        carrier,
        &predicate(PlaceId::new(2).unwrap(), Vec::new(), count)
    ));
    assert!(!domain_predicate_is_closed(
        types.iter(),
        carrier,
        &predicate(
            subject,
            vec![Segment::Field(StructuralFieldId::new(1).unwrap())],
            count
        )
    ));
    assert!(!domain_predicate_is_closed(
        types.iter(),
        carrier,
        &predicate(
            subject,
            Vec::new(),
            IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
        )
    ));
    types[0].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(count));
    assert!(!domain_predicate_is_closed(types.iter(), carrier, &valid));
}

#[test]
fn case_tests_and_payload_reads_close_over_the_subjects_declared_case() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let subject = PlaceId::new(1).unwrap();
    let low = StructuralFieldId::new(1).unwrap();
    let case_id = |id| StructuralCaseId::new(id).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    };
    let types = [
        declaration(
            scalar,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer)),
        ),
        declaration(
            carrier,
            StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: case_id(1),
                        identity: TerminalIdentityIndex(3),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        declaration_identity: Default::default(),
                        id: case_id(2),
                        identity: TerminalIdentityIndex(4),
                        fields: vec![StructuralFieldDeclaration {
                            declaration_identity: Default::default(),
                            inline_carrier: None,
                            id: low,
                            identity: TerminalIdentityIndex(5),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                                integer,
                            )),
                            storage: None,
                        }],
                    },
                ],
            },
        ),
    ];
    let predicate = |proposition| StructuralDomainPredicate {
        subject,
        proposition,
    };
    let closed = |proposition| {
        domain_predicate_is_closed(types.iter(), carrier, &predicate(proposition))
    };
    let membership = |root, case| Proposition::StructuralCaseMembership {
        subject: StructuralCaseSubject::new(root, Vec::new()),
        case: case_id(case),
    };
    let payload_at_most_five = |root, case| {
        Proposition::LessOrEqual(
            ScalarTerm::integer_field_path(
                root,
                vec![Segment::Case(case_id(case)), Segment::Field(low)],
                integer,
            ),
            ScalarTerm::integer(integer, IntegerValue::Unsigned(5)).unwrap(),
        )
    };

    assert!(closed(membership(subject, 2)));
    assert!(!domain_predicate_observes_no_storage(
        types.iter(),
        carrier,
        &predicate(membership(subject, 2))
    ));
    assert!(closed(payload_at_most_five(subject, 2)));

    let other = PlaceId::new(2).unwrap();
    assert!(!closed(membership(other, 2)));
    assert!(!closed(membership(subject, 3)));
    assert!(!closed(payload_at_most_five(other, 2)));
    assert!(!closed(payload_at_most_five(subject, 1)));
    assert!(!closed(payload_at_most_five(subject, 3)));
}
