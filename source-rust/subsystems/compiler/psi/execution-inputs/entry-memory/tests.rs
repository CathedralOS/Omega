use super::*;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, DomainSemanticId, MachineId, PlaceId,
    Proposition, QualifiedScalarType, ScalarQualificationSetId, StructuralDomainId,
    StructuralFieldId,
};
use terminal_psi::{
    BindingRelevance, ScalarFieldQualification, StructuralDomainDeclaration,
    StructuralFieldDeclaration, StructuralMultiplicity, StructuralRecordStorage,
    StructuralTypeDeclaration, TerminalIdentityIndex, TerminalModule,
};

#[test]
fn entry_bytes_must_establish_the_selected_records_own_requirements() {
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    let id = StructuralTypeId::new(1).unwrap();
    let first = StructuralFieldId::new(1).unwrap();
    let second = StructuralFieldId::new(2).unwrap();
    let field = |id| StructuralFieldDeclaration {
        declaration_identity: Default::default(),
        inline_carrier: None,
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
        storage: None,
    };
    let observed = |field| ScalarTerm::BooleanField {
        version: ContentPlaceVersion::Current,
        root: PlaceId::new(1).unwrap(),
        path: vec![CanonicalStructuralPathSegment::Field(field)],
    };
    module
        .types
        .structural_types
        .push(StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id,
            identity: TerminalIdentityIndex(0),
            quotient_carrier: None,
            owns_linear_claim: false,
            multiplicity: StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            nominal_cleanup: None,
            shape: StructuralTypeShape::Record {
                fields: vec![field(first), field(second)],
                storage: StructuralRecordStorage::Packed,
            },
            scalar_restriction: None,
            requires: vec![Proposition::Equal(observed(first), observed(second))],
        });
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
