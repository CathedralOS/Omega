use super::*;
use semantic_vocabulary::{
    ByteSequencePredicate, ContentAlgebra, ContentAlgebraKind, ContentDomainId,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    DomainSemanticId, MachineId, ScalarType,
};
use terminal_psi::{
    StructuralContentProjection, StructuralDomainPredicate, StructuralRecordStorage,
    StructuralTypeDeclaration, TerminalIdentityTableBuilder,
};

fn module(offset: u64) -> TerminalModule {
    let mut identities = TerminalIdentityTableBuilder::default();
    if offset != 0 {
        identities.intern("unrelated retained declaration");
    }
    let carrier_identity = identities.intern("named(Envelope)");
    let field_identity = identities.intern("ready");
    let domain_identity = identities.intern("Envelope::Issued@named(Envelope)");
    let issuer = identities.intern("Envelope::private_mint");
    let carrier = StructuralTypeId::new(offset + 1).unwrap();
    let field = StructuralFieldId::new(offset + 2).unwrap();
    let subject = PlaceId::new(offset + 3).unwrap();
    let semantic_domain = DomainSemanticId::new(offset + 4).unwrap();
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.identities = identities.build();
    module.structural_types.push(StructuralTypeDeclaration {
        id: carrier,
        identity: carrier_identity,
        quotient_carrier: None,
        owns_linear_claim: false,
        copyable: false,
        no_code_affine_cleanup: true,
        descriptor_cleanup: false,
        scalar_restriction: None,
        shape: StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: vec![StructuralFieldDeclaration {
                id: field,
                identity: field_identity,
                relevance: Default::default(),
                storage: None,
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            }],
        },
    });
    module.structural_domains.push(StructuralDomainDeclaration {
        id: StructuralDomainId::new(offset + 5).unwrap(),
        semantic_domain,
        identity: domain_identity,
        carrier,
        predicate: Some(StructuralDomainPredicate {
            subject,
            proposition: Proposition::Equal(
                ScalarTerm::BooleanField {
                    root: subject,
                    path: vec![CanonicalStructuralPathSegment::Field(field)],
                },
                ScalarTerm::Boolean(true),
            ),
        }),
        content_projection: Some(StructuralContentProjection {
            identity: ContentProjectionIdentity {
                domain: ContentDomainId::new(semantic_domain.get()).unwrap(),
                projection_report_fingerprint: offset,
            },
            algebra: ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Item".into(),
            },
            expression: ContentProjectionExpression::CountedQuantity(
                ContentProjectionScalar::Natural("1".into()),
            ),
        }),
        byte_predicate: None,
        establishment_routes: vec![StructuralEstablishmentRoute::ExactMachine { machine: issuer }],
    });
    module
}

#[test]
fn local_coordinates_rebind_only_complete_identical_theories() {
    let receiver = module(0);
    let provider = module(40);
    let joined = ComponentMemberships::join(&receiver, &provider).unwrap();
    let mut actual = vec![receiver.structural_domains[0].id];
    joined.present(&mut actual, true).unwrap();
    assert_eq!(actual, [provider.structural_domains[0].id]);
    joined.present(&mut actual, false).unwrap();
    assert_eq!(actual, [receiver.structural_domains[0].id]);
    let mut absent = vec![StructuralDomainId::new(999).unwrap()];
    assert!(joined.present(&mut absent, true).is_err());
}

#[test]
fn same_names_cannot_replace_predicate_content_or_issuer_theory() {
    let receiver = module(0);
    for mutation in 0..11 {
        let mut provider = module(40);
        let row = &mut provider.structural_domains[0];
        match mutation {
            0 => row.predicate = None,
            1 => row.predicate.as_mut().unwrap().proposition = Proposition::Falsehood,
            2 => row.byte_predicate = Some(ByteSequencePredicate::NoNul),
            3 => {
                row.content_projection.as_mut().unwrap().algebra.parameter = "DifferentUnit".into()
            }
            4 => {
                row.content_projection.as_mut().unwrap().expression =
                    ContentProjectionExpression::CountedQuantity(ContentProjectionScalar::Natural(
                        "2".into(),
                    ))
            }
            5 => {
                row.content_projection.as_mut().unwrap().identity.domain =
                    ContentDomainId::new(999).unwrap()
            }
            6 => {
                let StructuralEstablishmentRoute::ExactMachine { machine } =
                    row.establishment_routes[0]
                else {
                    unreachable!()
                };
                row.establishment_routes[0] = StructuralEstablishmentRoute::Requirement {
                    requirement: machine,
                };
            }
            7 => row.establishment_routes.clear(),
            8 => provider.structural_types[0].copyable = true,
            9 => {
                let StructuralTypeShape::Record { fields, .. } =
                    &mut provider.structural_types[0].shape
                else {
                    unreachable!()
                };
                fields[0].field_type = StructuralFieldType::Atomic(ScalarType::Boolean);
            }
            10 => {
                row.establishment_routes[0] = StructuralEstablishmentRoute::ExactMachine {
                    machine: provider.structural_types[0].identity,
                }
            }
            _ => unreachable!(),
        }
        assert!(
            ComponentMemberships::join(&receiver, &provider).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn opaque_and_explicit_empty_predicates_are_distinct() {
    let mut receiver = module(0);
    let mut provider = module(40);
    receiver.structural_domains[0].predicate = None;
    provider.structural_domains[0]
        .predicate
        .as_mut()
        .unwrap()
        .proposition = Proposition::Truth;
    assert!(ComponentMemberships::join(&receiver, &provider).is_err());
}

#[test]
fn missing_and_ambiguous_receiving_theories_never_drop_actual_tags() {
    let receiver = module(0);
    let mut provider = module(40);
    provider.structural_domains.clear();
    let joined = ComponentMemberships::join(&receiver, &provider).unwrap();
    let mut actual = vec![receiver.structural_domains[0].id];
    assert!(joined.present(&mut actual, true).is_err());
    let mut provider = module(40);
    let mut duplicate = provider.structural_domains[0].clone();
    duplicate.id = StructuralDomainId::new(999).unwrap();
    provider.structural_domains.push(duplicate);
    assert!(ComponentMemberships::join(&receiver, &provider).is_err());
}

#[test]
fn component_type_paths_round_trip_artifact_local_names() {
    let source = module(0);
    let destination = module(40);
    let source_type = &source.structural_types[0];
    let destination_type = &destination.structural_types[0];
    let StructuralTypeShape::Record {
        fields: source_fields,
        ..
    } = &source_type.shape
    else {
        unreachable!()
    };
    let StructuralTypeShape::Record {
        fields: destination_fields,
        ..
    } = &destination_type.shape
    else {
        unreachable!()
    };
    let presentation =
        join_component_types(&source, &destination, source_type.id, destination_type.id).unwrap();
    assert_eq!(presentation.source(), source_type.id);
    assert_eq!(presentation.destination(), destination_type.id);
    assert_eq!(
        presentation.present_type(source_type.id),
        Some(destination_type.id)
    );
    let mut path = vec![terminal_psi::StructuralPathSegment::Field(
        source_fields[0].identity,
    )];
    let original = path.clone();
    presentation.present_path(&mut path).unwrap();
    assert_eq!(
        path,
        [terminal_psi::StructuralPathSegment::Field(
            destination_fields[0].identity
        )]
    );
    presentation.original_path(&mut path).unwrap();
    assert_eq!(path, original);
    // The source index names an unrelated declaration in the destination.
    assert!(presentation.original_path(&mut original.clone()).is_err());
    assert!(
        presentation
            .present_path(&mut [terminal_psi::StructuralPathSegment::Referent])
            .is_err()
    );
}
