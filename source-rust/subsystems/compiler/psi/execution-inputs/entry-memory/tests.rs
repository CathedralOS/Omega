use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, DomainSemanticId, PlaceId, Proposition,
    QualifiedScalarType, ScalarQualificationSetId, StructuralDomainId, StructuralFieldId,
};
use terminal_psi::{
    BindingRelevance, ScalarFieldQualification, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralMultiplicity, StructuralRecordStorage,
    StructuralTypeDeclaration, TerminalIdentityIndex, TerminalModule,
};

fn record(types: &[StructuralFieldType], requires: Vec<Proposition>) -> TerminalModule {
    let mut module = TerminalModule::declarations();
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let identity = identities.intern("EntryRecord");
    let fields = types
        .iter()
        .enumerate()
        .map(|(position, field_type)| StructuralFieldDeclaration {
            declaration_identity: Default::default(),
            inline_carrier: None,
            id: StructuralFieldId::new(position as u64 + 1).unwrap(),
            identity: identities.intern(&format!("field_{position}")),
            relevance: BindingRelevance::Relevant,
            field_type: field_type.clone(),
            storage: None,
        })
        .collect();
    module.identities = identities.build();
    module
        .types
        .structural_types
        .push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: StructuralTypeId::new(1).unwrap(),
            identity,
            quotient_carrier: None,
            owns_linear_claim: false,
            multiplicity: StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            nominal_cleanup: None,
            shape: StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Packed,
            },
            scalar_restriction: None,
            requires,
        });
    module
}

#[test]
fn entry_bytes_must_establish_the_selected_records_own_requirements() {
    let first = StructuralFieldId::new(1).unwrap();
    let second = StructuralFieldId::new(2).unwrap();
    let observed = |field| ScalarTerm::BooleanField {
        version: ContentPlaceVersion::Current,
        root: PlaceId::new(1).unwrap(),
        path: vec![CanonicalStructuralPathSegment::Field(field)],
    };
    let mut module = record(
        &[
            StructuralFieldType::Scalar(ScalarType::Boolean),
            StructuralFieldType::Scalar(ScalarType::Boolean),
        ],
        vec![Proposition::Equal(observed(first), observed(second))],
    );
    let id = module.types.structural_types[0].id;
    // A named domain on this carrier is an independent qualification.
    // It is never asserted by this unqualified record's initializer.
    module
        .types
        .structural_domains
        .push(StructuralDomainDeclaration {
            id: StructuralDomainId::new(1).unwrap(),
            semantic_domain: DomainSemanticId::new(1).unwrap(),
            identity: TerminalIdentityIndex(3),
            carrier: id,
            subject_identity: TerminalIdentityIndex(0),
            complete_predicate_theory: true,
            predicate: None,
            byte_predicate: None,
            content_projection: None,
            establishment_routes: Some(Vec::new()),
            required_memberships: None,
            access_policy: None,
        });
    fn decode(
        module: &TerminalModule,
        bytes: &[u8],
    ) -> Result<Vec<PrivateEntryRamScalarInitialization>, String> {
        initialize_record(
            module,
            &terminal_codec::ClosedTypeCatalog::default(),
            &module.types.structural_types,
            &module.types.structural_types[0],
            bytes,
        )
    }
    let fields = decode(&module, &[1, 1]).unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].value, ScalarTerm::Boolean(true));
    assert_eq!(fields[1].byte_offset, 1);
    assert_eq!(
        decode(&module, &[1, 0]).unwrap_err(),
        "the decoded fields violate the record requirement"
    );
    assert!(decode(&module, &[2, 2]).is_err());

    let domain = semantic_vocabulary::ScalarDomainId::new(2).unwrap();
    let set = ScalarQualificationSetId::new(1);
    module
        .types
        .scalar_qualifications
        .domains
        .push(terminal_psi::ScalarDomainDeclaration {
            id: domain,
            carrier: ScalarType::Boolean,
            semantic_domain: DomainSemanticId::new(2).unwrap(),
            identity: TerminalIdentityIndex(4),
            required_memberships: None,
            predicate: None,
            establishment_routes: Some(Vec::new()),
        });
    module
        .types
        .scalar_qualifications
        .sets
        .push(terminal_psi::ScalarQualificationSet {
            id: set,
            domains: vec![domain],
        });
    module
        .types
        .scalar_qualifications
        .fields
        .push(ScalarFieldQualification {
            field: second,
            value_type: QualifiedScalarType {
                scalar_type: ScalarType::Boolean,
                qualifications: set,
            },
        });
    assert_eq!(
        decode(&module, &[1, 1]).unwrap_err(),
        "private entry RAM does not establish a declared scalar field domain"
    );
}

#[test]
fn entry_integer_bytes_must_satisfy_the_field_range_and_refinement() {
    use semantic_vocabulary::{BoundedIntegerType, IntegerType, ScalarRefinement};
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let literal = |n| ScalarTerm::Integer {
        scalar_type: integer,
        value: IntegerValue::Signed(n),
    };
    let bounded =
        BoundedIntegerType::new(integer, IntegerValue::Signed(-2), IntegerValue::Signed(9))
            .unwrap();
    let exact = ScalarRefinement::new(
        scalar,
        Proposition::Equal(
            ScalarTerm::value(ScalarRefinement::SUBJECT, scalar),
            literal(-3),
        ),
    )
    .unwrap();
    let module = record(
        &[
            StructuralFieldType::BoundedInteger(bounded),
            StructuralFieldType::RefinedScalar(exact),
        ],
        vec![],
    );
    let closed = terminal_codec::ClosedTerminalModule::new(module).unwrap();
    let module = closed.module();
    let decode = |bytes: &[u8]| {
        initialize_record(
            module,
            closed.closed_types(),
            &module.types.structural_types,
            &module.types.structural_types[0],
            bytes,
        )
    };
    for endpoint in [-2i8, 9] {
        let decoded = decode(&[endpoint as u8, (-3i8) as u8]).unwrap();
        assert_eq!(decoded[0].value, literal(i128::from(endpoint)));
        assert_eq!(decoded[1].value, literal(-3));
    }
    for bytes in [[(-3i8) as u8, (-3i8) as u8], [10, (-3i8) as u8], [9, 3]] {
        assert_eq!(
            decode(&bytes).unwrap_err(),
            "private entry RAM initializer violates its declaration"
        );
    }
}
