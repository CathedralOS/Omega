use super::*;
use crate::encoding::{PackagePolicyRecoveryError, PackagePolicyRecoveryLimits};
use crate::record::{
    PackagePolicyMachineParameterSignature, PackagePolicyTermination, PackagePolicyTypeParameter,
    PackagePolicyTypeParameterKind, PackageReviewCallableContract, PackageReviewContractExpression,
    PackageReviewContractFact, PackageReviewContractKind, PackageReviewContractStaticArgument,
    PackageReviewContractUnaryOperator, PackageReviewToolchainSourceIdentity,
};

const HEADER: &[u8] = b"OMEGA-EXTERNAL-SUPPLY-POLICY\0";

fn recover(bytes: &[u8]) -> PackagePolicyExternalExecutableSupply {
    PackagePolicyExternalExecutableSupply::recover_canonical(
        bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("recover complete external-supply policy")
}

fn assert_round_trip(policy: &PackagePolicyExternalExecutableSupply) {
    crate::encoding::assert_external_policy_text(policy);
    let encoded = policy
        .canonical_bytes()
        .expect("encode external-supply policy");
    let recovered = recover(&encoded);
    assert_eq!(&recovered, policy);
    assert_eq!(recovered.callable(), policy.callable());
    assert_eq!(recovered.signature(), policy.signature());
    assert_eq!(recovered.requirement(), policy.requirement());
    assert_eq!(recovered.binding(), policy.binding());
    assert_eq!(
        recovered
            .canonical_bytes()
            .expect("re-encode recovered policy"),
        encoded
    );
}

fn occurrence(bytes: &[u8], marker: &[u8]) -> usize {
    bytes
        .windows(marker.len())
        .position(|part| part == marker)
        .expect("fixture marker")
}

fn assert_rejected(bytes: &[u8]) {
    assert!(
        PackagePolicyExternalExecutableSupply::recover_canonical(
            bytes,
            PackagePolicyRecoveryLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn every_external_binding_and_foreign_locator_recovers_typed_policy() {
    let mut bindings = vec![
        PackageReviewExternalBinding::Import {
            library: "kernel32.dll".into(),
            symbol: "ExitProcess".into(),
        },
        PackageReviewExternalBinding::NormalizedSyscall(syscall()),
        PackageReviewExternalBinding::Syscall { number: 60 },
        PackageReviewExternalBinding::CompilerIntrinsic,
        PackageReviewExternalBinding::VtableSlot { index: 2 },
        PackageReviewExternalBinding::VtableField {
            field: "invoke".into(),
        },
        PackageReviewExternalBinding::TableFunction {
            field: "invoke".into(),
        },
    ];
    bindings.extend(
        locators()
            .into_iter()
            .map(|locator| PackageReviewExternalBinding::NormalizedImport(import(locator))),
    );
    for binding in bindings {
        assert_round_trip(&supply(binding).policy());
    }
    let mut normalized = import(locators().remove(0));
    normalized.producer_package = None;
    normalized.producer.owner =
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [0; 32],
        });
    let mut policy = supply(PackageReviewExternalBinding::NormalizedImport(normalized)).policy();
    policy.callable.owner =
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: [0; 32],
        });
    assert_round_trip(&policy);
}

#[test]
fn trait_and_operator_requirement_coordinates_survive_recovery() {
    let mut policy = supply(PackageReviewExternalBinding::CompilerIntrinsic).policy();
    policy.signature.lifetime_parameter_count = 2;
    policy.requirement =
        PackagePolicyExternalRequirement::Trait(PackagePolicyCallableConformance {
            trait_identity: nominal("Service"),
            requirement_identity: nominal("Service::invoke"),
            requirement_lifetime_partition: vec![0, 1, 0],
            trait_lifetime_arguments: vec![1, 0, 1],
            arguments: vec![value_type("i32"), value_type("u64")],
            alias: Some("provider".into()),
        });
    assert_round_trip(&policy);
    policy.requirement = PackagePolicyExternalRequirement::Operator {
        coordinate: PackageReviewOperatorCoordinate {
            identity: nominal("Math::identity"),
            parameter_dispatch: "(i32)".into(),
            result_dispatch: "i32".into(),
        },
        alias: None,
    };
    assert_round_trip(&policy);
}

#[test]
fn normalized_bindings_round_trip_every_canonical_target_profile() {
    for profile in omega_target::TargetProfile::ALL {
        let mut normalized_import = import(locators().remove(0));
        normalized_import.target = profile.identity().as_str().to_owned();
        assert_round_trip(
            &supply(PackageReviewExternalBinding::NormalizedImport(
                normalized_import,
            ))
            .policy(),
        );
        let mut normalized_syscall = syscall();
        normalized_syscall.target = profile.identity().as_str().to_owned();
        assert_round_trip(
            &supply(PackageReviewExternalBinding::NormalizedSyscall(
                normalized_syscall,
            ))
            .policy(),
        );
    }
}

#[test]
fn noncanonical_targets_reject_in_both_policy_directions() {
    let policies = [
        supply(PackageReviewExternalBinding::NormalizedImport(import(
            locators().remove(0),
        )))
        .policy(),
        supply(PackageReviewExternalBinding::NormalizedSyscall(syscall())).policy(),
    ];
    for policy in policies {
        let original_target = match policy.binding() {
            PackagePolicyExternalBinding::NormalizedImport { target, .. }
            | PackagePolicyExternalBinding::NormalizedSyscall { target, .. } => target.clone(),
            _ => unreachable!(),
        };
        let encoded = policy.canonical_bytes().expect("canonical target fixture");
        let target_start = occurrence(&encoded, original_target.as_bytes());
        for target in [
            "",
            "windows_x86_64",
            "windows_arm64",
            "omega.target-profile.v1:windows_arm64",
            "omega.target-profile.v1:Linux_x86_64",
            "omega.target-profile.v1:linux_x86_64 ",
            "host",
            "Host",
        ] {
            let mut invalid = policy.clone();
            match &mut invalid.binding {
                PackagePolicyExternalBinding::NormalizedImport {
                    target: retained, ..
                }
                | PackagePolicyExternalBinding::NormalizedSyscall {
                    target: retained, ..
                } => {
                    *retained = target.to_owned();
                }
                _ => unreachable!(),
            }
            assert!(
                invalid.canonical_bytes().is_err(),
                "writer accepted {target:?}"
            );
            let mut raw = encoded.clone();
            raw[target_start - 8..target_start].copy_from_slice(
                &u64::try_from(target.len())
                    .expect("short target fixture")
                    .to_le_bytes(),
            );
            raw.splice(
                target_start..target_start + original_target.len(),
                target.bytes(),
            );
            assert_eq!(
                PackagePolicyExternalExecutableSupply::recover_canonical(
                    &raw,
                    PackagePolicyRecoveryLimits::default(),
                ),
                Err(PackagePolicyRecoveryError::InvalidValue),
                "reader accepted {target:?}",
            );
        }
    }
}

fn structural_contract(depth: usize) -> PackagePolicyMachineParameterContract {
    if depth == 0 {
        return PackagePolicyMachineParameterContract::RequirementIdentity;
    }
    PackagePolicyMachineParameterContract::Structural(PackagePolicyMachineParameterSignature {
        lifetime_parameter_count: 1,
        type_parameters: vec![PackagePolicyTypeParameter {
            kind: PackagePolicyTypeParameterKind::Machine(structural_contract(depth - 1)),
            bounds: PackageReviewDataProperties {
                multiplicity: psi_language_semantics::Multiplicity::Affine,
                carry: None,
            },
        }],
        parameters: Vec::new(),
        return_type: Some(value_type("unit")),
        contracts: vec![PackageReviewCallableContract {
            kind: PackageReviewContractKind::Requires,
            result_case: None,
            binding: None,
            evidence_lane_position: None,
            fact: PackageReviewContractFact::Expression(PackageReviewContractExpression::Boolean(
                true,
            )),
        }],
        published_crash: Vec::new(),
        service_reach: vec![nominal("Service")],
        service_reach_is_installation_bound: true,
        synchronous_invocations: Vec::new(),
        suspends: false,
        blocks: false,
        termination: PackagePolicyTermination::Terminates {
            premises: Vec::new(),
        },
    })
}

#[test]
fn recursive_machine_contracts_and_static_arguments_recover_structurally() {
    let mut policy = supply(PackageReviewExternalBinding::CompilerIntrinsic).policy();
    policy.signature.static_parameters.push(static_parameter(
        PackagePolicyTypeParameterKind::Machine(structural_contract(4)),
    ));
    policy.signature.conformance_bounds[0].selected_conformance = Some(nominal("Selected"));
    policy.signature.conformance_bounds[0].selected_subject =
        Some(PackageReviewContractStaticArgument::Type(value_type("i32")));
    policy.signature.conformance_bounds[0].selected_arguments =
        vec![PackageReviewContractStaticArgument::GenericType {
            base: value_type("Container"),
            lifetime_arguments: vec![0],
            arguments: vec![
                PackageReviewContractStaticArgument::ConformanceApplication {
                    declaration: nominal("Conformance"),
                    arguments: vec![PackageReviewContractStaticArgument::ConstBoolean(true)],
                    subject: Box::new(PackageReviewContractStaticArgument::Type(value_type("i32"))),
                    trait_identity: nominal("Copyable"),
                    trait_arguments: vec![value_type("i32")],
                },
            ],
        }];
    assert_round_trip(&policy);
}

#[test]
fn header_versions_tags_boolean_utf8_and_zero_nominal_owners_reject() {
    let policy = supply(PackageReviewExternalBinding::CompilerIntrinsic).policy();
    let encoded = policy.canonical_bytes().expect("encode fixture");
    let callable_start = HEADER.len() + 2;
    let signature_start = occurrence(&encoded, b"Provider::invoke") + b"Provider::invoke".len();
    let first_static_tag = signature_start + 16;
    let requirement_tag = occurrence(&encoded, b"unit") + b"unit".len();
    let parameter_boolean = occurrence(&encoded, b"i32") + b"i32".len();
    let alias_option = occurrence(&encoded, b"chosen") - 9;
    for offset in [
        0,
        HEADER.len(),
        callable_start,
        first_static_tag,
        requirement_tag,
        parameter_boolean,
        alias_option,
        encoded.len() - 1,
    ] {
        let mut invalid = encoded.clone();
        invalid[offset] = 0xff;
        assert_rejected(&invalid);
    }
    let mut invalid = encoded.clone();
    invalid[callable_start + 1..callable_start + 33].fill(0);
    assert_rejected(&invalid);
    let mut invalid = encoded.clone();
    invalid[occurrence(&encoded, b"Provider::invoke")] = 0xff;
    assert_rejected(&invalid);

    let encoded = bytes(&supply(PackageReviewExternalBinding::NormalizedImport(
        import(locators().remove(0)),
    )));
    let mut invalid = encoded.clone();
    invalid[occurrence(&encoded, b"windows_x86_64") + b"windows_x86_64".len()] = 0xff;
    assert_rejected(&invalid);
}

#[test]
fn truncated_trailing_and_impossible_count_frames_reject() {
    let encoded = bytes(&supply(PackageReviewExternalBinding::CompilerIntrinsic));
    for length in 0..encoded.len() {
        assert_rejected(&encoded[..length]);
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_rejected(&trailing);
    let signature_start = occurrence(&encoded, b"Provider::invoke") + b"Provider::invoke".len();
    let static_count = signature_start + 8;
    for count in [u64::MAX, 65_537, 65_536] {
        let mut invalid = encoded.clone();
        invalid[static_count..static_count + 8].copy_from_slice(&count.to_le_bytes());
        assert_rejected(&invalid);
    }
    let mut invalid = encoded[..static_count + 8].to_vec();
    invalid[static_count..static_count + 8].copy_from_slice(&1_u64.to_le_bytes());
    assert_rejected(&invalid);
    let mut invalid = encoded.clone();
    invalid[HEADER.len() + 2 + 33..HEADER.len() + 2 + 41].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_rejected(&invalid);
}

#[test]
fn recovery_enforces_byte_field_aggregate_element_and_owned_storage_limits() {
    let encoded = bytes(&supply(PackageReviewExternalBinding::CompilerIntrinsic));
    let limits = [
        PackagePolicyRecoveryLimits::new(
            encoded.len() - 1,
            4 * 1024 * 1024,
            65_536,
            64 * 1024 * 1024,
            128,
        ),
        PackagePolicyRecoveryLimits::new(4 * 1024 * 1024, 3, 65_536, 64 * 1024 * 1024, 128),
        PackagePolicyRecoveryLimits::new(
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            3,
            64 * 1024 * 1024,
            128,
        ),
        PackagePolicyRecoveryLimits::new(4 * 1024 * 1024, 4 * 1024 * 1024, 65_536, 1, 128),
    ];
    for (limits, expected) in limits.into_iter().zip([
        PackagePolicyRecoveryError::InputTooLarge,
        PackagePolicyRecoveryError::FieldTooLarge,
        PackagePolicyRecoveryError::ElementLimitExceeded,
        PackagePolicyRecoveryError::AllocationLimitExceeded,
    ]) {
        assert_eq!(
            PackagePolicyExternalExecutableSupply::recover_canonical(&encoded, limits),
            Err(expected),
        );
    }
    let expanded = PackagePolicyRecoveryLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    let mut invalid = encoded.clone();
    let count = occurrence(&encoded, b"Provider::invoke") + b"Provider::invoke".len() + 8;
    invalid[count..count + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(PackagePolicyExternalExecutableSupply::recover_canonical(&invalid, expanded).is_err());
}

#[test]
fn canonical_reencoding_uses_the_same_owned_storage_budget() {
    let policy = PackagePolicyExternalExecutableSupply {
        callable: nominal("f"),
        signature: PackagePolicyExternalCallableSignature {
            lifetime_parameter_count: 0,
            static_parameters: Vec::new(),
            conformance_bounds: Vec::new(),
            parameters: Vec::new(),
            return_type: None,
        },
        requirement: PackagePolicyExternalRequirement::Operator {
            coordinate: PackageReviewOperatorCoordinate {
                identity: nominal("g"),
                parameter_dispatch: "()".into(),
                result_dispatch: String::new(),
            },
            alias: None,
        },
        binding: PackagePolicyExternalBinding::CompilerIntrinsic,
    };
    let encoded = policy
        .canonical_bytes()
        .expect("minimal valid field fixture");
    // The three nonempty strings request four bytes before canonical scratch.
    let owned_bytes = encoded.len() + 4;
    let limits = |owned_bytes| {
        PackagePolicyRecoveryLimits::new(4 * 1024 * 1024, 4 * 1024 * 1024, 65_536, owned_bytes, 128)
    };
    assert_eq!(
        PackagePolicyExternalExecutableSupply::recover_canonical(&encoded, limits(owned_bytes - 1)),
        Err(PackagePolicyRecoveryError::AllocationLimitExceeded),
    );
    assert_eq!(
        PackagePolicyExternalExecutableSupply::recover_canonical(&encoded, limits(owned_bytes)),
        Ok(policy),
    );
}

#[test]
fn recovery_and_writer_bound_recursive_machine_contract_depth() {
    let mut policy = supply(PackageReviewExternalBinding::CompilerIntrinsic).policy();
    policy.signature.conformance_bounds.clear();
    policy.signature.static_parameters = vec![static_parameter(
        PackagePolicyTypeParameterKind::Machine(structural_contract(8)),
    )];
    let encoded = policy
        .canonical_bytes()
        .expect("default depth accepts bounded fixture");
    assert_eq!(recover(&encoded), policy);
    let limits = PackagePolicyRecoveryLimits::new(
        4 * 1024 * 1024,
        4 * 1024 * 1024,
        65_536,
        64 * 1024 * 1024,
        2,
    );
    assert_eq!(
        PackagePolicyExternalExecutableSupply::recover_canonical(&encoded, limits),
        Err(PackagePolicyRecoveryError::NestingLimitExceeded),
    );
    policy.signature.static_parameters = vec![static_parameter(
        PackagePolicyTypeParameterKind::Machine(structural_contract(129)),
    )];
    assert!(policy.canonical_bytes().is_err());
}

fn expression_policy(unary_count: usize) -> PackagePolicyExternalExecutableSupply {
    let mut policy = supply(PackageReviewExternalBinding::CompilerIntrinsic).policy();
    let mut expression = PackageReviewContractExpression::Boolean(true);
    for _ in 0..unary_count {
        expression = PackageReviewContractExpression::Unary {
            operator: PackageReviewContractUnaryOperator::LogicalNot,
            operand: Box::new(expression),
        };
    }
    let PackagePolicyMachineParameterContract::Structural(mut contract) = structural_contract(1)
    else {
        unreachable!()
    };
    contract.contracts[0].fact = PackageReviewContractFact::Expression(expression);
    policy.signature.conformance_bounds.clear();
    policy.signature.static_parameters =
        vec![static_parameter(PackagePolicyTypeParameterKind::Machine(
            PackagePolicyMachineParameterContract::Structural(contract),
        ))];
    policy
}

#[test]
fn individual_expression_nodes_consume_the_aggregate_element_budget() {
    let policy = expression_policy(32);
    let encoded = policy.canonical_bytes().expect("bounded expression tree");
    assert_round_trip(&policy);
    let limits = PackagePolicyRecoveryLimits::new(
        4 * 1024 * 1024,
        4 * 1024 * 1024,
        16,
        64 * 1024 * 1024,
        128,
    );
    assert_eq!(
        PackagePolicyExternalExecutableSupply::recover_canonical(&encoded, limits),
        Err(PackagePolicyRecoveryError::ElementLimitExceeded),
    );
    let mut excessive = encoded.clone();
    let unary_chain = occurrence(&encoded, &[7, 1].repeat(32));
    excessive.splice(unary_chain..unary_chain, [7, 1].repeat(129));
    let unbounded_caller = PackagePolicyRecoveryLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
    );
    assert_eq!(
        PackagePolicyExternalExecutableSupply::recover_canonical(&excessive, unbounded_caller),
        Err(PackagePolicyRecoveryError::NestingLimitExceeded),
    );
}

#[test]
fn default_depth_frontier_recovers_and_rejects_on_a_two_megabyte_stack() {
    let shallow = expression_policy(32)
        .canonical_bytes()
        .expect("bounded source fixture");
    let unary_chain = occurrence(&shallow, &[7, 1].repeat(32));
    let mut permitted = shallow;
    // One enclosing machine contract plus 126 unary expressions and the
    // Boolean leaf reaches the format's 128-entry active-depth ceiling.
    permitted.splice(unary_chain..unary_chain, [7, 1].repeat(94));
    let mut excessive = permitted.clone();
    excessive.splice(unary_chain..unary_chain, [7, 1]);
    std::thread::Builder::new()
        .name("external-policy-bounded-stack".to_owned())
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let recovered = recover(&permitted);
            assert_eq!(
                recovered
                    .canonical_bytes()
                    .expect("re-encode depth frontier"),
                permitted
            );
            drop(recovered);
            assert_eq!(
                PackagePolicyExternalExecutableSupply::recover_canonical(
                    &excessive,
                    PackagePolicyRecoveryLimits::default(),
                ),
                Err(PackagePolicyRecoveryError::NestingLimitExceeded),
            );
        })
        .expect("spawn explicit small-stack recovery test")
        .join()
        .expect("small-stack recovery completed");
}
