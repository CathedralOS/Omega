use super::*;
use semantic_vocabulary::{
    Proposition, ScalarRefinement, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralPathSegment, StructuralRecordStorage, TerminalIdentityIndex,
};

fn scalar(raw: u64) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        id: StructuralTypeId::new(raw).unwrap(),
        identity: TerminalIdentityIndex(raw as u32),
        owns_linear_claim: false,
        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        quotient_carrier: None,
        scalar_restriction: None,
        shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
    }
}

fn fixture() -> (Vec<StructuralTypeDeclaration>, Vec<StructuralPathSegment>) {
    let mut root = scalar(1);
    root.shape = StructuralTypeShape::Record {
        storage: StructuralRecordStorage::Packed,
        fields: vec![StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            id: StructuralFieldId::new(1).unwrap(),
            identity: TerminalIdentityIndex(4),
            relevance: BindingRelevance::Relevant,
            storage: None,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            inline_carrier: Some(StructuralTypeId::new(2).unwrap()),
        }],
    };
    (
        vec![root, scalar(2), scalar(3)],
        vec![StructuralPathSegment::Field(TerminalIdentityIndex(4))],
    )
}

// Knowing what bytes satisfy does not establish an additional theory
// or install independent policy rules. Both literal and zero-default
// establishment must use the same pure-predicate classification.
#[test]
fn byte_literals_and_defaults_do_not_establish_extra_theory_or_access_rules() {
    use schema_reflection::DeclarationDescription;
    use schema_reflection::SchemaNode;
    use schema_reflection::SchemaNodeHandle;
    use schema_reflection::SchemaQueryAuthority;
    use schema_reflection::SchemaShape;
    use schema_reflection::SemanticSchemaGraph;
    use semantic_vocabulary::{
        DomainSemanticId, IntegerSign, IntegerType, PlaceId, StructuralDomainId,
    };
    use terminal_psi::{
        ByteSequencePredicate, StructuralDomainDeclaration, StructuralDomainPredicate,
        StructuralPathQualification,
    };
    let (mut types, path) = fixture();
    let array = types[1].id;
    let element = types[2].id;
    types[1].shape = StructuralTypeShape::FixedArray {
        element,
        length: terminal_psi::ArrayExtent::Literal(2),
    };
    types[2].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
    ));
    let StructuralTypeShape::Record { fields, .. } = &mut types[0].shape else {
        panic!("record");
    };
    fields[0].field_type = StructuralFieldType::Structural(array);
    fields[0].inline_carrier = None;
    let id = StructuralDomainId::new(1).unwrap();
    let pure = StructuralDomainDeclaration {
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: true,
        predicate: None,
        id,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: TerminalIdentityIndex(1),
        carrier: array,
        subject_identity: TerminalIdentityIndex(1),
        content_projection: None,
        byte_predicate: Some(ByteSequencePredicate::AsciiOnly),
        establishment_routes: None,
        access_policy: None,
    };
    let mut incomplete = pure.clone();
    incomplete.complete_predicate_theory = false;
    let mut extra = pure.clone();
    extra.predicate = Some(StructuralDomainPredicate {
        subject: PlaceId::new(1).unwrap(),
        proposition: Proposition::Falsehood,
    });
    let root = SchemaNodeHandle::from_index(1);
    let schema = SemanticSchemaGraph::from_nodes(
        vec![
            SchemaNode::Invalid,
            SchemaNode::Declaration(DeclarationDescription {
                owner_identity: "test::Bytes".into(),
                identity_is_hermetic: true,
                name: "Bytes".into(),
                shape: SchemaShape::Empty,
                supply: "Ordinary".into(),
                multiplicity: "Unrestricted".into(),
                type_parameters: Vec::new(),
                lifetime_parameters: Vec::new(),
                application_identity: Some("test::Bytes".into()),
                members: Vec::new(),
                retired_identities: Vec::new(),
            }),
        ],
        root,
        SchemaQueryAuthority::OwningScope {
            requester_identity: "test::Bytes".into(),
        },
    )
    .unwrap();
    let mut governed = pure.clone();
    governed.access_policy = Some(terminal_psi::StructuralDomainAccessPlan {
        schema,
        issuance: None,
        readers: vec![((root, Some(Vec::new())), Vec::new())],
        writers: Vec::new(),
        observations: Vec::new(),
        reach: Default::default(),
    });
    let mut complete_truth = pure.clone();
    complete_truth.predicate = Some(StructuralDomainPredicate {
        subject: PlaceId::new(1).unwrap(),
        proposition: Proposition::Truth,
    });
    let mut primitive_theory = pure.clone();
    let mut predicates = terminal_psi::ByteSequencePredicateSet::EMPTY;
    predicates.insert(ByteSequencePredicate::AsciiOnly);
    primitive_theory.predicate = Some(StructuralDomainPredicate {
        subject: PlaceId::new(1).unwrap(),
        proposition: crate::byte_predicate_requirement(
            predicates,
            terminal_psi::ByteSequencePredicateSet::EMPTY,
            PlaceId::new(1).unwrap(),
            &[],
        ),
    });
    for (domain, expected, facts_available) in [
        (pure, true, true),
        (complete_truth, true, true),
        (primitive_theory, true, true),
        (incomplete, false, false),
        (extra, false, false),
        (governed, false, true),
    ] {
        assert_eq!(
            crate::byte_literal_qualifications_hold(
                std::slice::from_ref(&domain),
                array,
                &[id],
                &[0, 0]
            ),
            expected
        );
        assert_eq!(
            crate::zeroed_projected_byte_qualifications_hold(
                types.iter(),
                std::iter::once(&domain),
                types[0].id,
                &[StructuralPathQualification {
                    path: path.clone(),
                    domain: id
                }]
            ),
            expected
        );
        // Projecting a primitive fact from an already established
        // domain is separate from establishing the complete domain.
        assert_eq!(
            crate::byte_qualification_predicates_in(std::iter::once(&domain), array, &[id])
                .is_some(),
            facts_available
        );
    }
}

#[test]
fn original_field_carrier_is_exact_among_equal_contracts() {
    let (mut declarations, path) = fixture();
    for _ in 0..2 {
        let root = declarations.iter().find(|row| row.id.get() == 1).unwrap();
        validate_inline_carriers(&declarations, root).unwrap();
        assert_eq!(
            crate::runtime_structural_path_tip(declarations.iter(), root.id, &path)
                .unwrap()
                .id
                .get(),
            2
        );
        declarations.swap(1, 2);
    }
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
        panic!("record")
    };
    fields[0].inline_carrier = Some(StructuralTypeId::new(99).unwrap());
    assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
    assert!(
        crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
            .is_none()
    );
}

#[test]
fn stronger_field_predicate_survives_a_weaker_declared_carrier() {
    let (mut declarations, path) = fixture();
    let restriction = StructuralFieldType::RefinedScalar(
        ScalarRefinement::new(
            ScalarType::Boolean,
            Proposition::Equal(
                ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Boolean),
                ScalarTerm::boolean(true),
            ),
        )
        .unwrap(),
    );
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
        panic!("record")
    };
    fields[0].field_type = restriction.clone();
    validate_inline_carriers(&declarations, &declarations[0]).unwrap();
    assert_eq!(
        crate::primitive_projection_restriction(
            declarations.iter(),
            declarations[0].id,
            &path
        ),
        Some(&restriction)
    );
    declarations[1].scalar_restriction = Some(restriction);
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
        panic!("record")
    };
    fields[0].field_type = StructuralFieldType::Scalar(ScalarType::Boolean);
    assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
}

#[test]
fn typed_field_presentation_keeps_the_requested_identity_or_refuses() {
    let (declarations, path) = fixture();
    let root = &declarations[0];
    let field =
        crate::runtime_structural_field(declarations.iter(), root.id, &path).unwrap();
    let expected = declarations[2].id;
    assert_eq!(
        inline_field_presentation(declarations.iter(), field, None, None),
        field.inline_carrier
    );
    assert_eq!(
        inline_field_presentation(
            declarations.iter(),
            field,
            Some(expected),
            field.field_type.canonical_leaf_type()
        ),
        Some(expected)
    );
    let missing = StructuralTypeId::new(99).unwrap();
    assert_eq!(
        inline_field_presentation(
            declarations.iter(),
            field,
            Some(missing),
            field.field_type.canonical_leaf_type()
        ),
        None
    );
    assert_eq!(
        crate::borrowed_primitive_projection_declaration(
            declarations.iter(),
            root.id,
            &path,
            Some(missing)
        ),
        None
    );
}

#[test]
fn atomic_field_carrier_is_storage_but_never_a_plain_scalar_read() {
    let (mut declarations, path) = fixture();
    let cell = semantic_vocabulary::AtomicCellType::Boolean;
    declarations[1].shape = StructuralTypeShape::AtomicCell(cell);
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
        panic!("record")
    };
    fields[0].field_type = StructuralFieldType::Atomic(ScalarType::Boolean);
    validate_inline_carriers(&declarations, &declarations[0]).unwrap();
    assert!(
        crate::canonical_inline_region(declarations.iter(), declarations[0].id, &path)
            .is_some()
    );
    assert_eq!(
        crate::atomic_cell_type(declarations.iter(), declarations[0].id, &path),
        Some(cell)
    );
    assert!(
        crate::primitive_projection_type(declarations.iter(), declarations[0].id, &path)
            .is_none()
    );
    assert!(
        crate::canonical_primitive_read_path(
            declarations.iter(),
            declarations[0].id,
            &path
        )
        .is_none()
    );
    declarations[1].shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
    assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
}

#[test]
fn inline_carrier_rejects_wrong_scalar_atomic_and_erased_fields() {
    let (mut declarations, _) = fixture();
    for field_type in [
        StructuralFieldType::Scalar(ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                8,
            )
            .unwrap(),
        )),
        StructuralFieldType::Atomic(ScalarType::Boolean),
        StructuralFieldType::Structural(declarations[1].id),
        StructuralFieldType::Erased {
            type_identity: TerminalIdentityIndex(4),
            scalar: None,
        },
    ] {
        let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
            panic!("record")
        };
        fields[0].field_type = field_type;
        assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
    }
}

#[test]
fn byte_field_keeps_exact_carrier_access_and_extent() {
    use terminal_psi::{ByteSequenceCarrier, StructuralAccess};
    let (mut declarations, path) = fixture();
    let carrier = ByteSequenceCarrier::BoundedView {
        access: Some(StructuralAccess::MutableBorrow),
        capacity: 6,
    };
    for declaration in &mut declarations[1..] {
        declaration.shape =
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                access: None,
                capacity: 6,
            });
    }
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
        panic!("record")
    };
    fields[0].field_type = StructuralFieldType::ByteSequence(carrier);
    validate_inline_carriers(&declarations, &declarations[0]).unwrap();
    assert_eq!(
        crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
            .unwrap()
            .id,
        declarations[1].id
    );
    declarations[1].shape = StructuralTypeShape::ByteSequence(carrier);
    validate_inline_carriers(&declarations, &declarations[0]).unwrap();
    assert_eq!(
        crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
            .unwrap()
            .id,
        declarations[1].id
    );
    for wrong in [
        ByteSequenceCarrier::BoundedView {
            access: Some(StructuralAccess::SharedBorrow),
            capacity: 6,
        },
        ByteSequenceCarrier::BoundedView {
            access: None,
            capacity: 7,
        },
        ByteSequenceCarrier::BoundedView {
            access: Some(StructuralAccess::MutableBorrow),
            capacity: 7,
        },
    ] {
        declarations[1].shape = StructuralTypeShape::ByteSequence(wrong);
        assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
        assert!(
            crate::runtime_structural_path_tip(
                declarations.iter(),
                declarations[0].id,
                &path
            )
            .is_none()
        );
    }
    let StructuralTypeShape::Record { fields, .. } = &mut declarations[0].shape else {
        panic!("record")
    };
    fields[0].field_type =
        StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        });
    declarations[1].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::MutableBorrow),
        });
    validate_inline_carriers(&declarations, &declarations[0]).unwrap();
    assert!(
        crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
            .is_some()
    );
    declarations[1].shape =
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        });
    assert!(validate_inline_carriers(&declarations, &declarations[0]).is_err());
    assert!(
        crate::runtime_structural_path_tip(declarations.iter(), declarations[0].id, &path)
            .is_none()
    );
}
