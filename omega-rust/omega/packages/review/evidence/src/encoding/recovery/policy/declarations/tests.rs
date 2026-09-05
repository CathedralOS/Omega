use super::super::{Error, PackagePolicyRecoveryLimits, reader::Reader};
use super::*;
use crate::encoding::encode::declarations::{
    encode_conformance_shape, encode_data_shape, encode_domain_shape, encode_trait_shape,
};
use crate::encoding::encode::text_test_support;
use crate::encoding::{
    PackageReviewEncodingError,
    encode::{self, encoder::Encoder},
};
use crate::record::*;
use language_semantics::{CarryPermission, DataSupplyMode, DomainPredicateBody, Multiplicity};

fn nominal(name: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [7; 32],
        }),
        path: name.to_owned(),
    }
}

fn value_type() -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: "u64".to_owned(),
    }
}

fn properties() -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: Multiplicity::Linear,
        carry: None,
    }
}

fn parameters() -> Vec<PackageReviewTypeParameter> {
    vec![
        PackageReviewTypeParameter {
            kind: PackageReviewTypeParameterKind::Type,
            bounds: properties(),
        },
        PackageReviewTypeParameter {
            kind: PackageReviewTypeParameterKind::Const(value_type()),
            bounds: properties(),
        },
        PackageReviewTypeParameter {
            kind: PackageReviewTypeParameterKind::Machine(
                PackageReviewMachineParameterContract::Nominal {
                    trait_identity: nominal("Interface"),
                    requirement_identity: nominal("Interface::run"),
                },
            ),
            bounds: properties(),
        },
        PackageReviewTypeParameter {
            kind: PackageReviewTypeParameterKind::Proposition(
                PackageReviewPropositionParameterSignature {
                    parameters: vec![PackageReviewPropositionParameterValue {
                        type_identity: value_type(),
                    }],
                },
            ),
            bounds: properties(),
        },
    ]
}

fn fact() -> PackageReviewContractFact {
    PackageReviewContractFact::Expression(PackageReviewContractExpression::Boolean(true))
}

fn contracts() -> Vec<PackageReviewCallableContract> {
    vec![PackageReviewCallableContract {
        kind: PackageReviewContractKind::Ensures,
        result_case: Some(PackageReviewResultCaseIdentity {
            result_data: nominal("Result"),
            result_case: nominal("Result::Ready"),
        }),
        binding: Some("fact".to_owned()),
        evidence_lane_position: Some(2),
        fact: fact(),
    }]
}

fn crash() -> Vec<PackageReviewCrashRoute> {
    vec![PackageReviewCrashRoute {
        cause: PackageReviewCrashCause::Abort,
        alternative_guards: vec![
            PackageReviewCrashRouteGuard::Truth,
            PackageReviewCrashRouteGuard::Predicate(PackageReviewCrashPredicate {
                canonical_bytes: vec![0, 255],
            }),
            PackageReviewCrashRouteGuard::Expression(PackageReviewContractExpression::Boolean(
                false,
            )),
        ],
    }]
}

fn interface() -> PackageReviewEvidenceInterface {
    PackageReviewEvidenceInterface {
        trait_identity: nominal("Interface"),
        lifetime_arguments: vec![1, 0],
        arguments: vec![value_type()],
        requirements: vec![PackageReviewEvidenceRequirement {
            declaring_trait: nominal("Parent"),
            declaring_trait_lifetime_arguments: vec![0],
            declaring_trait_arguments: vec![value_type()],
            requirement: nominal("Parent::run"),
        }],
    }
}

fn bytes<T>(
    value: &T,
    encode: impl Fn(&mut Encoder, &T) -> Result<(), PackageReviewEncodingError>,
) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(1024 * 1024);
    encode(&mut encoder, value).expect("bounded declaration fixture");
    encoder.finish().expect("fixture bytes")
}

fn roundtrip<T: std::fmt::Debug + PartialEq>(
    value: &T,
    encode: impl Fn(&mut Encoder, &T) -> Result<(), PackageReviewEncodingError>,
    decode: impl Fn(&mut Reader<'_>) -> Result<T, Error>,
) {
    let encoded = bytes(value, &encode);
    let mut reader = Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap();
    let recovered = decode(&mut reader).expect("complete typed inverse");
    reader.finish().unwrap();
    assert_eq!(&recovered, value);
    assert_eq!(bytes(&recovered, encode), encoded);
    for end in 0..encoded.len() {
        let mut reader =
            Reader::new(&encoded[..end], PackagePolicyRecoveryLimits::default()).unwrap();
        assert!(
            decode(&mut reader).is_err(),
            "truncated prefix {end} of {}",
            encoded.len()
        );
    }
    let mut trailing = encoded;
    trailing.push(0);
    let mut reader = Reader::new(&trailing, PackagePolicyRecoveryLimits::default()).unwrap();
    decode(&mut reader).unwrap();
    assert_eq!(reader.finish(), Err(Error::TrailingBytes));
}

fn data() -> PackageReviewDataShape {
    let field = PackageReviewDataField {
        identity: Some(9),
        name: "field".to_owned(),
        relevance: language_core::BindingRelevance::Relevant,
        type_identity: value_type(),
    };
    PackageReviewDataShape {
        identity: nominal("Data"),
        kind: PackageReviewDataKind::Ordinary,
        supply: DataSupplyMode::CheckedShape,
        lifetime_parameter_count: 2,
        type_parameters: parameters(),
        properties: properties(),
        zero_gated: true,
        invariants: vec![fact(), fact()],
        retired_identities: vec![3, 5],
        members: vec![
            PackageReviewDataMember::Field(field.clone()),
            PackageReviewDataMember::Variant {
                identity: None,
                name: "Case".to_owned(),
                payload: vec![PackageReviewDataField {
                    identity: None,
                    relevance: language_core::BindingRelevance::Erased,
                    ..field
                }],
                retired_payload_identities: vec![6, 8],
            },
        ],
    }
}

#[test]
fn data_kinds_supplies_members_and_erasure_roundtrip() {
    for kind in [
        PackageReviewDataKind::Ordinary,
        PackageReviewDataKind::Quotient {
            carrier: value_type(),
            relation: nominal("Equivalent"),
        },
    ] {
        for supply in [DataSupplyMode::CheckedShape, DataSupplyMode::BoundaryOpaque] {
            let shape = PackageReviewDataShape {
                kind: kind.clone(),
                supply,
                ..data()
            };
            roundtrip(&shape, encode_data_shape, data_shape);
            text_test_support::meaning(|encoder| encode_data_shape(encoder, &shape));
        }
    }
}

#[test]
fn domain_all_alias_roles_routes_and_presence_roundtrip() {
    for predicate_body in [DomainPredicateBody::Bodyless, DomainPredicateBody::Present] {
        for alias_expansion in [
            None,
            Some(vec![]),
            Some(vec![
                PackageReviewDomainAliasAtom::Declared(nominal("Base")),
                PackageReviewDomainAliasAtom::Carry(CarryPermission::AcrossSuspend),
                PackageReviewDomainAliasAtom::Carry(CarryPermission::AnyCpu),
                PackageReviewDomainAliasAtom::Carry(CarryPermission::AnyThread),
                PackageReviewDomainAliasAtom::Carry(CarryPermission::MovableAddress),
            ]),
        ] {
            for classification in [
                None,
                Some(PackageReviewDomainClassification::ProgressProfile),
            ] {
                let shape = PackageReviewDomainShape {
                    identity: nominal("Domain"),
                    type_parameters: parameters(),
                    target_type: value_type(),
                    index_arguments: vec![value_type()],
                    predicate_body,
                    predicate_facts: vec![fact()],
                    alias_expansion: alias_expansion.clone(),
                    classification,
                    semantic_roles: vec![
                        PackageReviewDomainSemanticRole::DenotationDimension,
                        PackageReviewDomainSemanticRole::ArithmeticPolicy,
                    ],
                    establishment_routes: [
                        PackageReviewDomainEstablishmentKind::CheckedRequirement,
                        PackageReviewDomainEstablishmentKind::BoundaryRequirement,
                    ]
                    .into_iter()
                    .map(|kind| PackageReviewDomainEstablishmentRoute {
                        kind,
                        trait_identity: nominal("Establish"),
                        requirement_identity: nominal("Establish::run"),
                    })
                    .collect(),
                };
                roundtrip(&shape, encode_domain_shape, domain_shape);
                text_test_support::meaning(|encoder| encode_domain_shape(encoder, &shape));
            }
        }
    }
}

#[test]
fn conformance_all_subjects_keep_complete_evidence_interfaces() {
    for subject in [
        PackageReviewConformanceSubject::Subjectless,
        PackageReviewConformanceSubject::TypeParameter(2),
        PackageReviewConformanceSubject::Nominal(nominal("Carrier")),
    ] {
        let shape = PackageReviewConformanceShape {
            identity: nominal("Conformance"),
            lifetime_parameter_count: 2,
            type_parameters: parameters(),
            subject,
            interface: interface(),
        };
        roundtrip(&shape, encode_conformance_shape, conformance_shape);
        text_test_support::meaning(|encoder| encode_conformance_shape(encoder, &shape));
    }
}

#[test]
fn proposition_all_public_bodies_and_binder_kinds_roundtrip() {
    for body in [
        PackageReviewPublicPropositionBody::Primitive,
        PackageReviewPublicPropositionBody::Witness(interface()),
        PackageReviewPublicPropositionBody::Transparent(fact()),
    ] {
        let shape = PackageReviewPropositionShape {
            identity: nominal("Proposition"),
            binders: [
                PackageReviewPropositionBinderKind::Type,
                PackageReviewPropositionBinderKind::Const(value_type()),
                PackageReviewPropositionBinderKind::Machine,
            ]
            .into_iter()
            .map(|kind| PackageReviewPropositionBinder {
                kind,
                bounds: properties(),
            })
            .collect(),
            parameter_types: vec![value_type()],
            body,
        };
        roundtrip(&shape, encode::encode_proposition_shape, proposition_shape);
        text_test_support::meaning(|encoder| encode::encode_proposition_shape(encoder, &shape));
    }
}

fn requirement() -> PackageReviewTraitRequirement {
    PackageReviewTraitRequirement {
        identity: nominal("Trait::run"),
        spelling: None,
        has_default_realization: true,
        lifetime_parameter_count: 2,
        type_parameters: parameters(),
        parameters: vec![PackageReviewTraitRequirementParameter {
            name: "input".to_owned(),
            type_identity: value_type(),
            is_const: true,
            is_mutable: false,
            is_self: true,
        }],
        return_type: value_type(),
        contracts: contracts(),
        published_crash: crash(),
        service_reach: vec![nominal("Service")],
        service_reach_is_installation_bound: true,
        synchronous_invocations: vec![
            PackageReviewSynchronousInvocation::Parameter(0),
            PackageReviewSynchronousInvocation::Service(nominal("Service")),
        ],
        suspends: true,
        blocks: true,
        termination: PackageReviewTermination::Terminates {
            premises: vec![PackageReviewProgressPremise {
                profile: nominal("Progress"),
                subject: PackageReviewProgressSubject::Receiver,
                projections: vec![nominal("Carrier::field")],
            }],
        },
    }
}

#[test]
fn trait_full_requirements_and_all_operator_spellings_roundtrip() {
    for spelling in (0..13)
        .map(|tag| {
            let encoded = [tag];
            Some(
                super::values::operator_spelling(
                    &mut Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap(),
                )
                .unwrap(),
            )
        })
        .chain([None])
    {
        let shape = PackageReviewTraitShape {
            identity: nominal("Trait"),
            is_boundary: true,
            lifetime_parameter_count: 2,
            type_parameters: parameters(),
            conformance_bounds: vec![PackageReviewConformanceBound {
                binder_ordinal: Some(0),
                subject_parameter: 0,
                selected_conformance: None,
                selected_lifetime_arguments: vec![],
                selected_arguments: vec![],
                selected_subject: None,
                trait_identity: nominal("Bound"),
                trait_lifetime_arguments: vec![1],
                arguments: vec![value_type()],
            }],
            parents: [
                PackageReviewTraitCompositionKind::Policy,
                PackageReviewTraitCompositionKind::ServiceReach,
            ]
            .into_iter()
            .map(|kind| PackageReviewTraitParent {
                kind,
                identity: nominal("Parent"),
                lifetime_arguments: vec![1, 0],
                arguments: vec![value_type()],
            })
            .collect(),
            requirements: vec![PackageReviewTraitRequirement {
                spelling,
                ..requirement()
            }],
        };
        roundtrip(&shape, encode_trait_shape, trait_shape);
        let operator = PackageReviewOperatorShape {
            coordinate: PackageReviewOperatorCoordinate {
                identity: nominal("Operator"),
                parameter_dispatch: "dispatch".to_owned(),
                result_dispatch: String::new(),
            },
            is_boundary: true,
            spelling,
            lifetime_parameter_count: 2,
            type_parameters: parameters(),
            parameters: vec![PackageReviewCallableParameter {
                name: "input".to_owned(),
                type_identity: value_type(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            }],
            return_type: value_type(),
            contracts: contracts(),
            published_crash: crash(),
        };
        roundtrip(&operator, encode::encode_operator_shape, operator_shape);
    }
}

#[test]
fn constants_keep_exact_value_encoding_and_string_bounds() {
    let shape = PackageReviewConstShape {
        identity: nominal("Constant"),
        declared_type: value_type(),
        canonical_value_encoding: "value\0λ".to_owned(),
    };
    roundtrip(&shape, encode::encode_const_shape, const_shape);
    text_test_support::meaning(|encoder| encode::encode_const_shape(encoder, &shape));
    let encoded = bytes(&shape, encode::encode_const_shape);
    let mut reader = Reader::new(
        &encoded,
        PackagePolicyRecoveryLimits::new(encoded.len(), 1, 100, 4096, 128),
    )
    .unwrap();
    assert_eq!(const_shape(&mut reader), Err(Error::FieldTooLarge));
}

#[test]
fn declaration_children_share_element_storage_and_depth_budgets() {
    let encoded = bytes(&data(), encode_data_shape);
    for (elements, owned, depth, expected) in [
        (0, 100_000, 128, Error::ElementLimitExceeded),
        (1000, 0, 128, Error::AllocationLimitExceeded),
        (1000, 100_000, 0, Error::NestingLimitExceeded),
    ] {
        let mut reader = Reader::new(
            &encoded,
            PackagePolicyRecoveryLimits::new(encoded.len(), encoded.len(), elements, owned, depth),
        )
        .unwrap();
        assert_eq!(data_shape(&mut reader), Err(expected));
    }
    let shape = PackageReviewPropositionShape {
        identity: nominal("P"),
        binders: vec![],
        parameter_types: vec![],
        body: PackageReviewPublicPropositionBody::Transparent(fact()),
    };
    let mut two = bytes(&shape, encode::encode_proposition_shape);
    two.extend_from_slice(&two.clone());
    let mut reader = Reader::new(
        &two,
        PackagePolicyRecoveryLimits::new(two.len(), two.len(), 1, 100_000, 128),
    )
    .unwrap();
    assert_eq!(proposition_shape(&mut reader).unwrap(), shape);
    assert_eq!(
        proposition_shape(&mut reader),
        Err(Error::ElementLimitExceeded)
    );
}

#[test]
fn malformed_declaration_tags_counts_and_utf8_reject() {
    let mut encoded = bytes(&data(), encode_data_shape);
    let kind_offset = 41 + "Data".len();
    encoded[kind_offset] = 2;
    assert_eq!(
        data_shape(&mut Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap()),
        Err(Error::InvalidTag)
    );
    encoded[kind_offset] = 0;
    encoded[kind_offset + 1] = 2;
    assert_eq!(
        data_shape(&mut Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap()),
        Err(Error::InvalidTag)
    );
    encoded[kind_offset + 1] = 0;
    encoded[kind_offset + 10..kind_offset + 18].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(
        data_shape(&mut Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap())
            .is_err()
    );
    for tag in [13, 255] {
        assert_eq!(
            super::values::operator_spelling(
                &mut Reader::new(&[tag], PackagePolicyRecoveryLimits::default()).unwrap()
            ),
            Err(Error::InvalidTag)
        );
    }
    let shape = PackageReviewConstShape {
        identity: nominal("X"),
        declared_type: value_type(),
        canonical_value_encoding: "x".to_owned(),
    };
    let mut encoded = bytes(&shape, encode::encode_const_shape);
    *encoded.last_mut().unwrap() = 255;
    assert_eq!(
        const_shape(&mut Reader::new(&encoded, PackagePolicyRecoveryLimits::default()).unwrap()),
        Err(Error::InvalidUtf8)
    );
}
