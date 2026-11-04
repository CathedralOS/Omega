use super::*;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, StructuralCaseId, StructuralCaseSubject,
    StructuralFieldId,
};
use terminal_psi::{
    BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeShape, TerminalIdentityIndex,
};

// `Interval { case Empty; case Range(lo: u64) }` with a domain such as
// `(self in Interval::Range) && self.lo <= 5`. A case test and a payload read
// close only over their own subject and a case the sum declares, and both
// observe the sum's storage.
#[test]
fn case_tests_and_payload_reads_close_over_the_subjects_declared_case() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let carrier = StructuralTypeId::new(2).unwrap();
    let subject = PlaceId::new(1).unwrap();
    let low = StructuralFieldId::new(1).unwrap();
    let case_id = |id| StructuralCaseId::new(id).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        quotient_carrier: None,
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        copyable: true,
        descriptor_cleanup: true,
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
                        id: case_id(1),
                        identity: TerminalIdentityIndex(3),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: case_id(2),
                        identity: TerminalIdentityIndex(4),
                        fields: vec![StructuralFieldDeclaration {
                            inline_carrier: None,
                            id: low,
                            identity: TerminalIdentityIndex(5),
                            relevance: BindingRelevance::Relevant,
                            field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer)),
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
    let closed =
        |proposition| domain_predicate_is_closed(types.iter(), carrier, &predicate(proposition));
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
    // `lo` belongs to Range, not Empty, and no case 3 exists.
    assert!(!closed(payload_at_most_five(subject, 1)));
    assert!(!closed(payload_at_most_five(subject, 3)));
}
