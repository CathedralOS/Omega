use super::tests::{fixture, nominal_fixture, recover, unchecked_bytes};
use crate::record::*;

fn value_type() -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: "u64".into(),
    }
}

fn parameter(kind: PackagePolicyTypeParameterKind) -> PackagePolicyTypeParameter {
    PackagePolicyTypeParameter {
        kind,
        bounds: PackageReviewDataProperties {
            multiplicity: language_semantics::Multiplicity::Unrestricted,
            carry: None,
        },
    }
}

fn static_parameters(
    machine: PackagePolicyMachineParameterContract,
) -> Vec<PackagePolicyTypeParameter> {
    vec![
        parameter(PackagePolicyTypeParameterKind::Type),
        parameter(PackagePolicyTypeParameterKind::Const(value_type())),
        parameter(PackagePolicyTypeParameterKind::Machine(machine)),
        parameter(PackagePolicyTypeParameterKind::Proposition(
            PackageReviewPropositionParameterSignature {
                parameters: vec![PackageReviewPropositionParameterValue {
                    type_identity: value_type(),
                }],
            },
        )),
    ]
}

fn contract(
    kind: PackageReviewContractKind,
    fact: PackageReviewContractFact,
) -> PackageReviewCallableContract {
    PackageReviewCallableContract {
        kind,
        result_case: None,
        binding: None,
        evidence_lane_position: None,
        fact,
    }
}

fn result_equals_input() -> PackageReviewContractExpression {
    PackageReviewContractExpression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Builtin,
        operator: PackageReviewContractBinaryOperator::Equal,
        left: Box::new(PackageReviewContractExpression::Result),
        right: Box::new(PackageReviewContractExpression::Parameter(0)),
    }
}

fn scope_call(offset: u32, lifetime: u32) -> PackageReviewContractExpression {
    use PackageReviewContractStaticArgument as Argument;
    PackageReviewContractExpression::Call {
        receiver: None,
        target: PackageReviewContractCallTarget::Nominal(nominal_fixture("valid")),
        static_arguments: vec![
            Argument::GenericTypeBinder(offset),
            Argument::GenericConstBinder(offset + 1),
            Argument::GenericMachineBinder(offset + 2),
            Argument::GenericType {
                base: PackageReviewTypeIdentity {
                    canonical: "Window".into(),
                },
                lifetime_arguments: vec![0, lifetime],
                arguments: vec![
                    Argument::GenericTypeBinder(0),
                    Argument::GenericConstBinder(1),
                ],
            },
        ],
        evidence_arguments: Vec::new(),
        arguments: vec![PackageReviewContractExpression::Parameter(0)],
    }
}

fn contracts(offset: u32, lifetime: u32) -> Vec<PackageReviewCallableContract> {
    use PackageReviewContractFact as Fact;
    use PackageReviewContractKind as Kind;
    let guarantee = contract(Kind::Ensures, Fact::Expression(result_equals_input()));
    vec![
        contract(
            Kind::Requires,
            Fact::Expression(scope_call(offset, lifetime)),
        ),
        guarantee.clone(),
        contract(
            Kind::Requires,
            Fact::PropositionParameter(PackageReviewPropositionParameterApplication {
                binder_ordinal: offset + 3,
                arguments: vec![PackageReviewContractExpression::Parameter(0)],
            }),
        ),
        guarantee,
    ]
}

fn static_fixture() -> PackagePolicyCallables {
    let mut policy = fixture();
    let nested = PackagePolicyMachineParameterSignature {
        lifetime_parameter_count: 1,
        type_parameters: static_parameters(
            PackagePolicyMachineParameterContract::RequirementIdentity,
        ),
        parameters: vec![PackageReviewMachineParameterValue {
            name: "input".into(),
            type_identity: value_type(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        }],
        return_type: Some(value_type()),
        // Nested statics follow all four outer binders; its lifetime follows the two outer lifetimes.
        contracts: contracts(4, 2),
        published_crash: vec![PackagePolicyCrashRoute {
            cause: PackageReviewCrashCause::Trap,
            alternative_guards: vec![PackagePolicyCrashGuard::Expression(
                PackageReviewContractExpression::Boolean(false),
            )],
        }],
        service_reach: vec![nominal_fixture("Console")],
        service_reach_is_installation_bound: false,
        synchronous_invocations: vec![PackageReviewSynchronousInvocation::Parameter(0)],
        suspends: false,
        blocks: true,
        termination: PackagePolicyTermination::NoGuarantee,
    };
    policy.callables[0].type_parameters =
        static_parameters(PackagePolicyMachineParameterContract::Structural(nested));
    policy.callables[0].contracts = contracts(0, 1);
    policy
}

fn nested(policy: &mut PackagePolicyCallables) -> &mut PackagePolicyMachineParameterSignature {
    let PackagePolicyTypeParameterKind::Machine(PackagePolicyMachineParameterContract::Structural(
        signature,
    )) = &mut policy.callables[0].type_parameters[2].kind
    else {
        panic!("fixture structural machine")
    };
    signature
}

fn arguments(
    contract: &mut PackageReviewCallableContract,
) -> &mut Vec<PackageReviewContractStaticArgument> {
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Call {
        static_arguments,
        ..
    }) = &mut contract.fact
    else {
        panic!("fixture call")
    };
    static_arguments
}

fn rejects(policy: &PackagePolicyCallables) {
    assert!(
        policy.canonical_bytes().is_err(),
        "writer accepted {policy:?}"
    );
    assert!(
        recover(&unchecked_bytes(policy)).is_err(),
        "reader accepted {policy:?}"
    );
}

#[test]
fn every_static_kind_nested_scope_and_ordered_duplicate_contract_round_trips() {
    let policy = static_fixture();
    let bytes = policy.canonical_bytes().unwrap();
    let recovered = recover(&bytes).unwrap();
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    assert_eq!(recovered.callables[0].contracts.len(), 4);
    assert_eq!(
        recovered.callables[0].contracts[1],
        recovered.callables[0].contracts[3]
    );
    let mut reordered = policy.clone();
    reordered.callables[0].contracts.swap(0, 1);
    let reordered_bytes = reordered.canonical_bytes().unwrap();
    assert_ne!(reordered_bytes, bytes);
    assert_eq!(recover(&reordered_bytes).unwrap(), reordered);
}

#[test]
fn static_argument_categories_ordinals_and_proposition_arity_are_checked() {
    use PackageReviewContractStaticArgument as Argument;
    for invalid in [
        Argument::GenericTypeBinder(1),
        Argument::GenericConstBinder(2),
        Argument::GenericMachineBinder(3),
        Argument::GenericTypeBinder(4),
    ] {
        let mut policy = static_fixture();
        arguments(&mut policy.callables[0].contracts[0])[0] = invalid;
        rejects(&policy);
    }
    for invalid in [
        Argument::GenericTypeBinder(5),
        Argument::GenericConstBinder(6),
        Argument::GenericMachineBinder(7),
        Argument::GenericTypeBinder(8),
    ] {
        let mut policy = static_fixture();
        arguments(&mut nested(&mut policy).contracts[0])[0] = invalid;
        rejects(&policy);
    }
    for ordinal in [0, 2, 4] {
        let mut policy = static_fixture();
        let PackageReviewContractFact::PropositionParameter(application) =
            &mut policy.callables[0].contracts[2].fact
        else {
            unreachable!()
        };
        application.binder_ordinal = ordinal;
        rejects(&policy);
    }
    let mut policy = static_fixture();
    let PackageReviewContractFact::PropositionParameter(application) =
        &mut nested(&mut policy).contracts[2].fact
    else {
        unreachable!()
    };
    application.arguments.clear();
    rejects(&policy);
}

#[test]
fn lifetimes_are_bounded_by_the_containing_and_nested_telescope() {
    for is_nested in [false, true] {
        let mut policy = static_fixture();
        let contract = if is_nested {
            &mut nested(&mut policy).contracts[0]
        } else {
            &mut policy.callables[0].contracts[0]
        };
        let PackageReviewContractStaticArgument::GenericType {
            lifetime_arguments, ..
        } = &mut arguments(contract)[3]
        else {
            unreachable!()
        };
        lifetime_arguments[1] = if is_nested { 3 } else { 2 };
        rejects(&policy);
    }
}

#[test]
fn result_is_only_available_to_postconditions_with_a_result() {
    for is_nested in [false, true] {
        let mut policy = static_fixture();
        let contract = if is_nested {
            &mut nested(&mut policy).contracts[0]
        } else {
            &mut policy.callables[0].contracts[0]
        };
        contract.fact = PackageReviewContractFact::Expression(result_equals_input());
        rejects(&policy);

        let mut policy = static_fixture();
        if is_nested {
            nested(&mut policy).published_crash[0].alternative_guards =
                vec![PackagePolicyCrashGuard::Expression(result_equals_input())];
        } else {
            policy.callables[0].checked_crash.published[0].alternative_guards =
                vec![PackagePolicyCrashGuard::Expression(result_equals_input())];
        }
        rejects(&policy);
    }
    let mut policy = static_fixture();
    policy.callables[0].return_type = None;
    rejects(&policy);
}
