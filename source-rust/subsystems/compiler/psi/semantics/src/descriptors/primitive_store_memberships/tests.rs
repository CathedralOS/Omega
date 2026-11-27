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

#[test]
fn a_prepared_plan_answers_declared_restriction_memberships_without_reproving() {
    use semantic_vocabulary::{
        ContentPlaceVersion, DomainSemanticId, PlaceId, Proposition, ScalarTerm, StructuralFieldId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralDomainPredicate, StructuralFieldDeclaration,
        StructuralRecordStorage,
    };
    let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = StructuralTypeId::new(1).unwrap();
    let record = StructuralTypeId::new(2).unwrap();
    let bounded = |maximum| {
        StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                byte,
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(maximum),
            )
            .unwrap(),
        )
    };
    let field = |identity: u32, field_type| StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        id: StructuralFieldId::new(u64::from(identity)).unwrap(),
        identity: TerminalIdentityIndex(identity),
        relevance: BindingRelevance::Relevant,
        storage: None,
        field_type,
        inline_carrier: Some(scalar),
    };
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
    let types_with = |restriction: StructuralFieldType| {
        vec![
            declaration(
                scalar,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(byte)),
            ),
            declaration(
                record,
                StructuralTypeShape::Record {
                    storage: StructuralRecordStorage::Packed,
                    fields: vec![
                        field(4, restriction),
                        field(5, bounded(200)),
                        field(6, StructuralFieldType::Scalar(ScalarType::Integer(byte))),
                    ],
                },
            ),
        ]
    };
    let subject = PlaceId::new(1).unwrap();
    let identity = StructuralDomainId::new(1).unwrap();
    let small = StructuralDomainDeclaration {
        subject_identity: Default::default(),
        access_policy: None,
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: Some(StructuralDomainPredicate {
            subject,
            proposition: Proposition::LessOrEqual(
                ScalarTerm::IntegerField {
                    version: ContentPlaceVersion::Current,
                    root: subject,
                    path: Vec::new(),
                    scalar_type: byte,
                },
                ScalarTerm::Integer {
                    scalar_type: byte,
                    value: IntegerValue::Unsigned(100),
                },
            ),
        }),
        id: identity,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: TerminalIdentityIndex(3),
        carrier: scalar,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: None,
    };
    let carrier_only = [declaration(
        scalar,
        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(byte)),
    )];
    let required = scalar::refinement(carrier_only.iter(), &small).unwrap();
    let types = types_with(StructuralFieldType::RefinedScalar(required.clone()));
    let path = |identity| {
        [StructuralPathSegment::Field(TerminalIdentityIndex(
            identity,
        ))]
    };
    let plan = IntrinsicMemberships::prepare(types.iter(), std::iter::once(&small));
    let index = crate::descriptor_lifetimes::StructuralTypeIndex::new(&types);
    // The plan answers exactly as proving the declared restriction again.
    let required = StructuralFieldType::RefinedScalar(required);
    assert!(!crate::scalar_field_entails(&bounded(200), &required));
    for (field, expected) in [(4, true), (5, false), (6, false)] {
        assert_eq!(
            plan.entails(types.iter(), record, &path(field), identity),
            expected,
            "field {field} under a scan"
        );
        assert_eq!(
            plan.entails(&index, record, &path(field), identity),
            expected,
            "field {field} under the type index"
        );
    }
    assert!(!plan.entails(
        types.iter(),
        record,
        &path(4),
        StructuralDomainId::new(2).unwrap()
    ));

    let mut incomplete = small.clone();
    incomplete.complete_predicate_theory = false;
    let duplicated = [small.clone(), small.clone()];
    for plan in [
        IntrinsicMemberships::prepare(types.iter(), std::iter::once(&incomplete)),
        IntrinsicMemberships::prepare(types.iter(), duplicated.iter()),
    ] {
        assert!(!plan.entails(types.iter(), record, &path(4), identity));
    }

    // A plan answers for the declarations it was prepared from. A view
    // whose restriction no longer entails the domain gets its own plan.
    let widened = types_with(bounded(50));
    assert!(!crate::scalar_field_entails(&bounded(50), &required));
    let replanned = IntrinsicMemberships::prepare(widened.iter(), std::iter::once(&small));
    assert!(!replanned.entails(widened.iter(), record, &path(4), identity));
    assert_ne!(plan, replanned);
}
