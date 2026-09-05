use super::super::PackagePolicyRecoveryLimits;
use super::*;
use crate::encoding::encode::text_test_support::{self, Component};
use crate::encoding::encode::{
    encode_policy_machine_contract, encode_public_api, encoder::Encoder,
};
use crate::record::*;

fn identity(name: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [9; 32],
        }),
        path: name.to_owned(),
    }
}

fn value_type() -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: "Unit".to_owned(),
    }
}

fn properties() -> PackageReviewDataProperties {
    PackageReviewDataProperties {
        multiplicity: language_semantics::Multiplicity::Affine,
        carry: None,
    }
}

fn routes() -> Vec<PackagePolicyCrashRoute> {
    vec![PackagePolicyCrashRoute {
        cause: PackageReviewCrashCause::Trap,
        alternative_guards: vec![PackagePolicyCrashGuard::Expression(
            PackageReviewContractExpression::Boolean(false),
        )],
    }]
}

fn termination() -> PackagePolicyTermination {
    use effects::provider_plan::ServiceProgressEstablishmentRouteKind as Kind;
    PackagePolicyTermination::Terminates {
        premises: vec![PackagePolicyProgressPremise {
            profile: identity("Progress"),
            subject: PackageReviewProgressSubject::Parameter(0),
            projections: vec![identity("Input::scheduler")],
            establishment_routes: vec![
                PackagePolicyServiceProgressRoute {
                    kind: Kind::CheckedRequirement,
                    requirement_owner: identity("Establish"),
                    requirement: identity("Establish::run"),
                },
                PackagePolicyServiceProgressRoute {
                    kind: Kind::BoundaryRequirement,
                    requirement_owner: identity("Host"),
                    requirement: identity("Host::run"),
                },
            ],
        }],
    }
}

fn signature() -> PackagePolicyMachineParameterSignature {
    PackagePolicyMachineParameterSignature {
        lifetime_parameter_count: 1,
        type_parameters: vec![],
        parameters: vec![PackageReviewMachineParameterValue {
            name: "input".to_owned(),
            type_identity: value_type(),
            is_const: true,
            is_mutable: false,
            is_self: false,
        }],
        return_type: None,
        contracts: vec![],
        published_crash: routes(),
        service_reach: vec![identity("Host")],
        service_reach_is_installation_bound: true,
        synchronous_invocations: vec![PackageReviewSynchronousInvocation::Parameter(0)],
        suspends: true,
        blocks: false,
        termination: termination(),
    }
}

fn parameters() -> Vec<PackagePolicyTypeParameter> {
    [
        PackagePolicyTypeParameterKind::Type,
        PackagePolicyTypeParameterKind::Const(value_type()),
        PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::RequirementIdentity,
        ),
        PackagePolicyTypeParameterKind::Machine(PackagePolicyMachineParameterContract::Nominal {
            trait_identity: identity("Interface"),
            requirement_identity: identity("Interface::run"),
        }),
        PackagePolicyTypeParameterKind::Machine(PackagePolicyMachineParameterContract::Structural(
            signature(),
        )),
        PackagePolicyTypeParameterKind::Proposition(PackageReviewPropositionParameterSignature {
            parameters: vec![PackageReviewPropositionParameterValue {
                type_identity: value_type(),
            }],
        }),
    ]
    .into_iter()
    .map(|kind| PackagePolicyTypeParameter {
        kind,
        bounds: properties(),
    })
    .collect()
}

pub(in crate::encoding::recovery::policy) fn fixture() -> PackagePolicyPublicApi {
    PackagePolicyPublicApi {
        traits: vec![PackagePolicyTraitShape {
            identity: identity("Trait"),
            is_boundary: true,
            lifetime_parameter_count: 1,
            type_parameters: parameters(),
            conformance_bounds: vec![],
            parents: vec![],
            requirements: vec![PackagePolicyTraitRequirement {
                identity: identity("Trait::run"),
                spelling: Some(language_core::OperatorSpelling::Index),
                has_default_realization: true,
                lifetime_parameter_count: 1,
                type_parameters: parameters(),
                parameters: vec![],
                return_type: None,
                contracts: vec![],
                published_crash: routes(),
                service_reach: vec![identity("Host")],
                service_reach_is_installation_bound: true,
                synchronous_invocations: vec![PackageReviewSynchronousInvocation::Service(
                    identity("Host"),
                )],
                suspends: true,
                blocks: false,
                termination: termination(),
            }],
        }],
        conformances: vec![PackagePolicyConformanceShape {
            identity: identity("Conformance"),
            lifetime_parameter_count: 1,
            type_parameters: parameters(),
            subject: PackageReviewConformanceSubject::TypeParameter(0),
            interface: PackageReviewEvidenceInterface {
                trait_identity: identity("Trait"),
                lifetime_arguments: vec![0],
                arguments: vec![value_type()],
                requirements: vec![],
            },
        }],
        domains: vec![PackagePolicyDomainShape {
            identity: identity("Domain"),
            type_parameters: parameters(),
            target_type: value_type(),
            index_arguments: vec![],
            predicate_body: language_semantics::DomainPredicateBody::Present,
            predicate_facts: vec![PackageReviewContractFact::Expression(
                PackageReviewContractExpression::Boolean(true),
            )],
            alias_expansion: Some(vec![PackageReviewDomainAliasAtom::Carry(
                language_semantics::CarryPermission::AcrossSuspend,
            )]),
            classification: Some(PackageReviewDomainClassification::ProgressProfile),
            semantic_roles: vec![PackageReviewDomainSemanticRole::ArithmeticPolicy],
            establishment_routes: vec![],
        }],
        propositions: vec![PackageReviewPropositionShape {
            identity: identity("Proposition"),
            binders: vec![],
            parameter_types: vec![],
            body: PackageReviewPublicPropositionBody::Primitive,
        }],
        consts: vec![PackageReviewConstShape {
            identity: identity("Const"),
            declared_type: value_type(),
            canonical_value_encoding: "unit".to_owned(),
        }],
        operators: vec![PackagePolicyOperatorShape {
            coordinate: PackageReviewOperatorCoordinate {
                identity: identity("Operator"),
                parameter_dispatch: "dispatch".to_owned(),
                result_dispatch: String::new(),
            },
            is_boundary: false,
            spelling: Some(language_core::OperatorSpelling::Add),
            lifetime_parameter_count: 1,
            type_parameters: parameters(),
            parameters: vec![],
            return_type: None,
            contracts: vec![],
            published_crash: routes(),
        }],
        data: vec![PackagePolicyDataShape {
            identity: identity("Data"),
            kind: PackageReviewDataKind::Ordinary,
            supply: language_semantics::DataSupplyMode::CheckedShape,
            lifetime_parameter_count: 1,
            type_parameters: parameters(),
            properties: properties(),
            zero_gated: false,
            invariants: vec![],
            retired_identities: vec![1],
            members: vec![PackageReviewDataMember::Variant {
                identity: Some(2),
                name: "Case".to_owned(),
                payload: vec![],
                retired_payload_identities: vec![3],
            }],
        }],
    }
}

fn encode(api: &PackagePolicyPublicApi) -> Vec<u8> {
    let mut encoder = Encoder::policy_bounded(1_000_000);
    encode_public_api(&mut encoder, api).unwrap();
    encoder.finish().unwrap()
}

fn decode(
    bytes: &[u8],
    limits: PackagePolicyRecoveryLimits,
) -> Result<PackagePolicyPublicApi, Error> {
    let mut reader = Reader::new(bytes, limits)?;
    let api = public_api(&mut reader)?;
    reader.finish()?;
    Ok(api)
}

#[test]
fn complete_public_api_text_retains_all_declaration_families() {
    let mut api = fixture();
    text_test_support::component(Component::PublicApi(&api));
    api.traits[0]
        .conformance_bounds
        .push(PackageReviewConformanceBound {
            binder_ordinal: Some(0),
            subject_parameter: 0,
            selected_conformance: Some(identity("Selected")),
            selected_lifetime_arguments: vec![0],
            selected_arguments: vec![PackageReviewContractStaticArgument::GenericTypeBinder(0)],
            selected_subject: Some(PackageReviewContractStaticArgument::GenericTypeBinder(0)),
            trait_identity: identity("Interface"),
            trait_lifetime_arguments: vec![0],
            arguments: vec![value_type()],
        });
    for carry in [
        language_semantics::CarryPolicy {
            suspension: language_semantics::CarrySuspension::Forbidden,
            cpu: language_semantics::CarryCpu::Origin,
            host_thread: language_semantics::CarryHostThread::Origin,
            address: language_semantics::CarryAddress::Stable,
        },
        language_semantics::CarryPolicy {
            suspension: language_semantics::CarrySuspension::Allowed,
            cpu: language_semantics::CarryCpu::Any,
            host_thread: language_semantics::CarryHostThread::Any,
            address: language_semantics::CarryAddress::Movable,
        },
    ] {
        api.data[0].properties.carry = Some(carry);
        text_test_support::component(Component::PublicApi(&api));
    }
}

#[test]
fn nested_static_contract_text_retains_promises_at_supported_depth() {
    let mut contract = PackagePolicyMachineParameterContract::Structural(signature());
    for _ in 0..24 {
        contract = PackagePolicyMachineParameterContract::Structural(
            PackagePolicyMachineParameterSignature {
                type_parameters: vec![PackagePolicyTypeParameter {
                    kind: PackagePolicyTypeParameterKind::Machine(contract),
                    bounds: properties(),
                }],
                ..signature()
            },
        );
    }
    text_test_support::meaning(|encoder| encode_policy_machine_contract(encoder, &contract));
    let mut encoder = Encoder::policy_bounded(1_000_000);
    encode_policy_machine_contract(&mut encoder, &contract).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(signatures::machine_contract(&mut reader).unwrap(), contract);
    reader.finish().unwrap();
}

#[test]
fn normalized_public_api_roundtrips_all_families_and_nested_full_promises() {
    let api = fixture();
    let bytes = encode(&api);
    let recovered = decode(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(recovered, api);
    assert_eq!(encode(&recovered), bytes);
    for length in 0..bytes.len() {
        assert!(
            decode(&bytes[..length], PackagePolicyRecoveryLimits::default()).is_err(),
            "prefix {length}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode(&trailing, PackagePolicyRecoveryLimits::default()),
        Err(Error::TrailingBytes)
    );
}

#[test]
fn absent_results_differ_from_unit_and_typed_route_owners_remain_observable() {
    let api = fixture();
    let baseline = encode(&api);
    let mut result = api.clone();
    result.traits[0].requirements[0].return_type = Some(value_type());
    assert_ne!(encode(&result), baseline);
    assert_eq!(
        decode(&encode(&result), PackagePolicyRecoveryLimits::default()).unwrap(),
        result
    );
    let mut operator = api.clone();
    operator.operators[0].return_type = Some(value_type());
    assert_ne!(encode(&operator), baseline);
    let mut nested = api.clone();
    let PackagePolicyTypeParameterKind::Machine(PackagePolicyMachineParameterContract::Structural(
        signature,
    )) = &mut nested.data[0].type_parameters[4].kind
    else {
        panic!("structural machine fixture")
    };
    signature.return_type = Some(value_type());
    assert_ne!(encode(&nested), baseline);
    let mut progress = api.clone();
    let PackagePolicyTermination::Terminates { premises } =
        &mut progress.traits[0].requirements[0].termination
    else {
        panic!("progress fixture")
    };
    premises[0].establishment_routes[0].requirement.owner =
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [3; 32],
        });
    assert_ne!(encode(&progress), baseline);
    assert_eq!(
        decode(&encode(&progress), PackagePolicyRecoveryLimits::default()).unwrap(),
        progress
    );
    let mut guards = api;
    guards.operators[0].published_crash[0].alternative_guards =
        vec![PackagePolicyCrashGuard::Truth];
    assert_ne!(encode(&guards), baseline);
}

#[test]
fn public_api_children_use_one_resource_budget() {
    let bytes = encode(&fixture());
    for (elements, owned, depth, error) in [
        (0, 1_000_000, 128, Error::ElementLimitExceeded),
        (1000, 0, 128, Error::AllocationLimitExceeded),
        (1000, 1_000_000, 0, Error::NestingLimitExceeded),
    ] {
        assert_eq!(
            decode(
                &bytes,
                PackagePolicyRecoveryLimits::new(bytes.len(), bytes.len(), elements, owned, depth)
            ),
            Err(error)
        );
    }
    let contract =
        PackagePolicyMachineParameterContract::Structural(PackagePolicyMachineParameterSignature {
            type_parameters: vec![PackagePolicyTypeParameter {
                kind: PackagePolicyTypeParameterKind::Machine(
                    PackagePolicyMachineParameterContract::Structural(signature()),
                ),
                bounds: properties(),
            }],
            ..signature()
        });
    let mut encoder = Encoder::policy_bounded(1_000_000);
    encode_policy_machine_contract(&mut encoder, &contract).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut reader = Reader::new(
        &bytes,
        PackagePolicyRecoveryLimits::new(bytes.len(), bytes.len(), 1000, 1_000_000, 1),
    )
    .unwrap();
    assert_eq!(
        signatures::machine_contract(&mut reader),
        Err(Error::NestingLimitExceeded)
    );
    let mut reader = Reader::new(&bytes, PackagePolicyRecoveryLimits::default()).unwrap();
    assert_eq!(signatures::machine_contract(&mut reader).unwrap(), contract);
    reader.finish().unwrap();
}

#[test]
fn normalized_signature_and_aggregate_unknown_tags_reject() {
    assert_eq!(
        signatures::machine_contract(
            &mut Reader::new(&[3], PackagePolicyRecoveryLimits::default()).unwrap()
        ),
        Err(Error::InvalidTag)
    );
    assert_eq!(
        signatures::type_parameter(
            &mut Reader::new(&[4], PackagePolicyRecoveryLimits::default()).unwrap()
        ),
        Err(Error::InvalidTag)
    );
    let mut bytes = encode(&fixture());
    bytes[..8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(decode(&bytes, PackagePolicyRecoveryLimits::default()).is_err());
}
