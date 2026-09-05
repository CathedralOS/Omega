use super::*;

fn package() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([3; 32]).unwrap()
}
fn identity(name: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(package()),
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
        multiplicity: language_semantics::Multiplicity::Unrestricted,
        carry: None,
    }
}
fn parameter(kind: PackagePolicyTypeParameterKind) -> PackagePolicyTypeParameter {
    PackagePolicyTypeParameter {
        kind,
        bounds: properties(),
    }
}
fn fact(value: PackageReviewContractExpression) -> PackageReviewContractFact {
    PackageReviewContractFact::Expression(value)
}
fn api() -> PackagePolicyPublicApi {
    PackagePolicyPublicApi {
        traits: vec![],
        conformances: vec![],
        domains: vec![],
        propositions: vec![],
        consts: vec![],
        operators: vec![],
        data: vec![],
    }
}
fn data() -> PackagePolicyDataShape {
    PackagePolicyDataShape {
        identity: identity("Data"),
        kind: PackageReviewDataKind::Ordinary,
        supply: language_semantics::DataSupplyMode::CheckedShape,
        lifetime_parameter_count: 1,
        type_parameters: vec![parameter(PackagePolicyTypeParameterKind::Type)],
        properties: properties(),
        zero_gated: false,
        invariants: vec![fact(PackageReviewContractExpression::DomainSubject)],
        retired_identities: vec![1],
        members: vec![PackageReviewDataMember::Field(PackageReviewDataField {
            identity: Some(2),
            name: "field".to_owned(),
            relevance: language_core::BindingRelevance::Relevant,
            type_identity: value_type(),
        })],
    }
}
fn signature() -> PackagePolicyMachineParameterSignature {
    PackagePolicyMachineParameterSignature {
        lifetime_parameter_count: 1,
        type_parameters: vec![parameter(PackagePolicyTypeParameterKind::Type)],
        parameters: vec![PackageReviewMachineParameterValue {
            name: "value".to_owned(),
            type_identity: value_type(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        }],
        return_type: None,
        contracts: vec![],
        published_crash: vec![],
        service_reach: vec![],
        service_reach_is_installation_bound: false,
        synchronous_invocations: vec![],
        suspends: false,
        blocks: false,
        termination: PackagePolicyTermination::NoGuarantee,
    }
}
fn contract(value: PackageReviewContractExpression) -> PackageReviewCallableContract {
    PackageReviewCallableContract {
        kind: PackageReviewContractKind::Ensures,
        result_case: None,
        binding: None,
        evidence_lane_position: None,
        fact: fact(value),
    }
}
fn generic_call(binder: u32, lifetime: u32) -> PackageReviewContractExpression {
    PackageReviewContractExpression::Call {
        receiver: None,
        target: PackageReviewContractCallTarget::Nominal(identity("callee")),
        static_arguments: vec![PackageReviewContractStaticArgument::GenericType {
            base: value_type(),
            lifetime_arguments: vec![lifetime],
            arguments: vec![PackageReviewContractStaticArgument::GenericTypeBinder(
                binder,
            )],
        }],
        evidence_arguments: vec![],
        arguments: vec![],
    }
}

#[test]
fn root_owned_sorted_unique_collections_are_required() {
    let mut value = api();
    value.data.push(data());
    assert!(value.validate_canonical_structure(package()).is_ok());
    value.data.push(data());
    assert!(value.validate_canonical_structure(package()).is_err());
    value.data.pop();
    value.data[0].identity.owner =
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [7; 32],
        });
    assert!(value.validate_canonical_structure(package()).is_err());
}

#[test]
fn declaration_subjects_and_proposition_telescope_are_distinct() {
    let mut value = api();
    value.data.push(data());
    value.domains.push(PackagePolicyDomainShape {
        identity: identity("Domain"),
        type_parameters: vec![],
        target_type: value_type(),
        index_arguments: vec![],
        predicate_body: language_semantics::DomainPredicateBody::Present,
        predicate_facts: vec![fact(PackageReviewContractExpression::DomainSubject)],
        alias_expansion: None,
        classification: None,
        semantic_roles: vec![],
        establishment_routes: vec![],
    });
    value.propositions.push(PackageReviewPropositionShape {
        identity: identity("Proposition"),
        binders: vec![PackageReviewPropositionBinder {
            kind: PackageReviewPropositionBinderKind::Type,
            bounds: properties(),
        }],
        parameter_types: vec![value_type()],
        body: PackageReviewPublicPropositionBody::Transparent(fact(
            PackageReviewContractExpression::Parameter(0),
        )),
    });
    assert!(value.validate_canonical_structure(package()).is_ok());
    value.propositions[0].body = PackageReviewPublicPropositionBody::Transparent(fact(
        PackageReviewContractExpression::GenericBinder(0),
    ));
    assert!(value.validate_canonical_structure(package()).is_ok());
    value.propositions[0].body = PackageReviewPublicPropositionBody::Transparent(fact(
        PackageReviewContractExpression::DomainSubject,
    ));
    assert!(value.validate_canonical_structure(package()).is_err());
    value.propositions.clear();
    value.data[0].invariants = vec![fact(PackageReviewContractExpression::Parameter(0))];
    assert!(value.validate_canonical_structure(package()).is_err());
    value.data[0] = data();
    value.domains[0].predicate_facts = vec![fact(PackageReviewContractExpression::Result)];
    assert!(value.validate_canonical_structure(package()).is_err());
}

#[test]
fn nested_static_lifetimes_kinds_and_optional_result_are_checked() {
    let mut value = api();
    let mut declaration = data();
    let mut nested = signature();
    nested.contracts = vec![contract(generic_call(0, 0)), contract(generic_call(2, 1))];
    declaration
        .type_parameters
        .push(parameter(PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(nested.clone()),
        )));
    value.data.push(declaration);
    assert!(value.validate_canonical_structure(package()).is_ok());
    let replace = |value: &mut PackagePolicyPublicApi, nested| {
        value.data[0].type_parameters[1].kind = PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(nested),
        );
    };
    nested.contracts.push(contract(generic_call(1, 0)));
    replace(&mut value, nested.clone());
    assert!(
        value.validate_canonical_structure(package()).is_err(),
        "machine binder is not a type"
    );
    nested.contracts.pop();
    nested.contracts.push(contract(generic_call(0, 2)));
    replace(&mut value, nested.clone());
    assert!(
        value.validate_canonical_structure(package()).is_err(),
        "lifetime escapes outer plus local scope"
    );
    nested.contracts.pop();
    nested
        .contracts
        .push(contract(PackageReviewContractExpression::Result));
    replace(&mut value, nested.clone());
    assert!(value.validate_canonical_structure(package()).is_err());
    nested.return_type = Some(value_type());
    replace(&mut value, nested.clone());
    assert!(value.validate_canonical_structure(package()).is_ok());
    nested.contracts.last_mut().unwrap().kind = PackageReviewContractKind::Requires;
    replace(&mut value, nested);
    assert!(value.validate_canonical_structure(package()).is_err());
}

#[test]
fn retired_member_ids_and_local_payload_uniqueness_are_checked() {
    let mut value = api();
    value.data.push(data());
    value.data[0].retired_identities.push(2);
    assert!(value.validate_canonical_structure(package()).is_err());
    value.data[0] = data();
    let duplicate = value.data[0].members[0].clone();
    value.data[0].members.push(duplicate);
    assert!(value.validate_canonical_structure(package()).is_err());
    value.data[0] = data();
    let PackageReviewDataMember::Field(field) = value.data[0].members.pop().unwrap() else {
        panic!("field fixture")
    };
    value.data[0]
        .members
        .push(PackageReviewDataMember::Variant {
            identity: Some(4),
            name: "Variant".to_owned(),
            payload: vec![field.clone(), field],
            retired_payload_identities: vec![],
        });
    assert!(value.validate_canonical_structure(package()).is_err());
}

#[test]
fn crash_and_progress_have_no_result_or_escaping_parameter_scope() {
    let mut value = api();
    let mut declaration = data();
    let mut nested = signature();
    nested.return_type = Some(value_type());
    nested.published_crash = vec![PackagePolicyCrashRoute {
        cause: PackageReviewCrashCause::Trap,
        alternative_guards: vec![PackagePolicyCrashGuard::Expression(
            PackageReviewContractExpression::Result,
        )],
    }];
    declaration
        .type_parameters
        .push(parameter(PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(nested.clone()),
        )));
    value.data.push(declaration);
    assert!(value.validate_canonical_structure(package()).is_err());
    nested.published_crash[0].alternative_guards = vec![PackagePolicyCrashGuard::Truth];
    nested.termination = PackagePolicyTermination::Terminates {
        premises: vec![PackagePolicyProgressPremise {
            profile: identity("Progress"),
            subject: PackageReviewProgressSubject::Parameter(1),
            projections: vec![],
            establishment_routes: vec![],
        }],
    };
    value.data[0].type_parameters[1].kind = PackagePolicyTypeParameterKind::Machine(
        PackagePolicyMachineParameterContract::Structural(nested),
    );
    assert!(value.validate_canonical_structure(package()).is_err());
}

#[test]
fn recursive_signature_validation_stops_before_unbounded_descent() {
    let mut nested = PackagePolicyMachineParameterContract::RequirementIdentity;
    for _ in 0..129 {
        nested = PackagePolicyMachineParameterContract::Structural(
            PackagePolicyMachineParameterSignature {
                type_parameters: vec![parameter(PackagePolicyTypeParameterKind::Machine(nested))],
                ..signature()
            },
        );
    }
    let mut value = api();
    let mut declaration = data();
    declaration.type_parameters = vec![parameter(PackagePolicyTypeParameterKind::Machine(nested))];
    value.data.push(declaration);
    assert!(value.validate_canonical_structure(package()).is_err());
}

#[test]
fn recursive_owner_frontiers_do_not_count_nonrecursive_wrappers() {
    fn unary(count: usize) -> PackageReviewContractExpression {
        let mut value = PackageReviewContractExpression::Boolean(true);
        for _ in 0..count {
            value = PackageReviewContractExpression::Unary {
                operator: PackageReviewContractUnaryOperator::LogicalNot,
                operand: Box::new(value),
            };
        }
        value
    }
    for count in [126, 127] {
        for crash in [false, true] {
            let mut nested = signature();
            if crash {
                nested.published_crash = vec![PackagePolicyCrashRoute {
                    cause: PackageReviewCrashCause::Trap,
                    alternative_guards: vec![PackagePolicyCrashGuard::Expression(unary(count))],
                }];
            } else {
                nested.contracts = vec![contract(unary(count))];
            }
            let parameters = vec![parameter(PackagePolicyTypeParameterKind::Machine(
                PackagePolicyMachineParameterContract::Structural(nested),
            ))];
            // Machine + unary chain + Boolean leaf is exactly 128 owners.
            assert_eq!(
                signatures::parameters(&scope(&parameters, 0), 0).is_ok(),
                count == 126
            );
        }
    }
    for count in [127, 128] {
        let mut nested = PackagePolicyMachineParameterContract::RequirementIdentity;
        for _ in 0..count {
            nested = PackagePolicyMachineParameterContract::Structural(
                PackagePolicyMachineParameterSignature {
                    type_parameters: vec![parameter(PackagePolicyTypeParameterKind::Machine(
                        nested,
                    ))],
                    ..signature()
                },
            );
        }
        let parameters = vec![parameter(PackagePolicyTypeParameterKind::Machine(nested))];
        assert_eq!(
            signatures::parameters(&scope(&parameters, 0), 0).is_ok(),
            count == 127
        );
    }
    for count in [126, 127] {
        let mut argument = PackageReviewContractStaticArgument::Type(value_type());
        for _ in 0..count {
            argument = PackageReviewContractStaticArgument::GenericType {
                base: value_type(),
                lifetime_arguments: vec![],
                arguments: vec![argument],
            };
        }
        let expression = PackageReviewContractExpression::Call {
            receiver: None,
            target: PackageReviewContractCallTarget::Nominal(identity("callee")),
            static_arguments: vec![argument],
            evidence_arguments: vec![],
            arguments: vec![],
        };
        assert_eq!(
            contracts::fact(&fact(expression), &scope(&[], 0), 0).is_ok(),
            count == 126
        );
    }
}
