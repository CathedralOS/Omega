use super::*;

pub(super) fn nominal_fixture(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(
            psi_core::PackageKeyIdentity::from_digest([17; 32]).unwrap(),
        ),
        path: path.into(),
    }
}

pub(in crate::encoding::recovery::policy) fn fixture() -> PackagePolicyCallables {
    let identity = nominal_fixture("inspect");
    let PackageReviewNominalOwner::Package(package) = identity.owner else {
        unreachable!()
    };
    let value_type = PackageReviewTypeIdentity {
        canonical: "u64".into(),
    };
    let mut callable = PackagePolicyCallable {
        role: PackagePolicyCallableRole::Public, identity, supply: PackageReviewCallableSupply::CheckedBody,
        lifetime_parameter_count: 2,
        type_parameters: Vec::new(), conformance_bounds: Vec::new(),
        parameters: ["left", "right"].into_iter().map(|name| PackageReviewCallableParameter {
            name: name.into(), type_identity: value_type.clone(), is_const: false, is_mutable: false, is_self: false,
        }).collect(),
        return_type: Some(value_type),
        conformances: vec![PackagePolicyCallableConformance {
            trait_identity: nominal_fixture("Inspect"), requirement_identity: nominal_fixture("Inspect::inspect"),
            requirement_lifetime_partition: vec![0], trait_lifetime_arguments: vec![1], arguments: Vec::new(), alias: Some("selected".into()),
        }],
        operator_realizations: Vec::new(), contracts: Vec::new(), declared_service_reach: Some(Vec::new()),
        checked_service_reach: PackageReviewCheckedServiceReach::CheckedBody { realized: Vec::new(), concrete: Vec::new() },
        unresolved_installation_reaches: Vec::new(), declared_synchronous_invocations: Some(Vec::new()), realized_synchronous_invocations: Vec::new(),
        capability_flows: Vec::new(), reachable_capability_flows: Vec::new(), checked_may_suspend: false, checked_may_block: false,
        declared_may_suspend: Some(false), declared_may_block: Some(false), declared_termination: Some(PackagePolicyTermination::NoGuarantee),
        checked_termination: PackagePolicyTermination::Terminates { premises: vec![PackagePolicyProgressPremise {
            profile: nominal_fixture("Progress"), subject: PackageReviewProgressSubject::Parameter(0), projections: Vec::new(),
            establishment_routes: vec![PackagePolicyServiceProgressRoute {
                kind: omega_effects::provider_plan::ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                requirement_owner: nominal_fixture("ProgressSource"), requirement: nominal_fixture("ProgressSource::establish"),
            }],
        }] },
        checked_crash: PackagePolicyCrash {
            inferred: PackagePolicyInferredCrash::Unknown,
            interface: PackageReviewCrashInterface::PublishedCeiling,
            published: vec![PackagePolicyCrashRoute { cause: PackageReviewCrashCause::Trap, alternative_guards: vec![PackagePolicyCrashGuard::Expression(PackageReviewContractExpression::Boolean(false))] }],
            structural_runtime_requirements: Some(vec![PackageReviewBooleanExpression::IntegerComparison {
                kind: PackageReviewIntegerComparisonKind::Equal,
                left: Box::new(PackageReviewScalarExpression::Parameter { position: 0, primitive_type: PackageReviewPrimitiveType::U64 }),
                right: Box::new(PackageReviewScalarExpression::Parameter { position: 1, primitive_type: PackageReviewPrimitiveType::U64 }),
            }]),
        },
        mutation: PackagePolicyMutation { completeness: PackageReviewWriteFrameCompleteness::Complete, paths: vec!["$P0.field".into()] },
    };
    callable.capability_flows = [
        psi_effects::CapabilityFlowKind::Uses,
        psi_effects::CapabilityFlowKind::Returns,
        psi_effects::CapabilityFlowKind::Acquires,
        psi_effects::CapabilityFlowKind::Stores,
        psi_effects::CapabilityFlowKind::Derives,
    ]
    .into_iter()
    .map(|kind| PackagePolicyCapabilityFlow {
        capability: nominal_fixture("Capability"),
        kind,
    })
    .collect();
    callable.reachable_capability_flows = callable.capability_flows.clone();
    PackagePolicyCallables {
        package,
        target: omega_target::TargetProfile::LinuxX64,
        callables: vec![callable],
    }
}

pub(super) fn recover(bytes: &[u8]) -> Result<PackagePolicyCallables, Error> {
    PackagePolicyCallables::recover_canonical(bytes, PackagePolicyRecoveryLimits::default())
}

pub(super) fn unchecked_bytes(policy: &PackagePolicyCallables) -> Vec<u8> {
    let mut encoder = crate::encoding::encode::encoder::Encoder::policy_bounded(4 * 1024 * 1024);
    encoder.fixed_bytes(CALLABLE_POLICY_MAGIC);
    encoder.u16(PACKAGE_CALLABLE_POLICY_VERSION);
    encoder.package_identity(policy.package);
    encoder.string(policy.target.identity().as_str()).unwrap();
    encoder
        .sequence(
            &policy.callables,
            crate::encoding::encode::encode_policy_callable,
        )
        .unwrap();
    encoder.finish().unwrap()
}

#[test]
fn callable_meaning_recovers_without_old_source_or_derivation_coordinates() {
    let policy = fixture();
    let bytes = policy.canonical_bytes().unwrap();
    assert_eq!(recover(&bytes).unwrap(), policy);
    assert_eq!(policy.callables[0].capability_flows.len(), 5);
    for target in omega_target::TargetProfile::ALL {
        let mut empty = policy.clone();
        empty.target = target;
        empty.callables.clear();
        assert_eq!(recover(&empty.canonical_bytes().unwrap()).unwrap(), empty);
    }
}

#[test]
fn policy_preserves_empty_absent_opaque_and_lifetime_selection_distinctions() {
    let original = fixture();
    let bytes = original.canonical_bytes().unwrap();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.callables[0].return_type = None;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].conformances[0].trait_lifetime_arguments = vec![0];
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_crash.published.clear();
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_crash.published.clear();
    changed.callables[0].checked_crash.inferred =
        PackagePolicyInferredCrash::Complete { causes: Vec::new() };
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_crash.published.clear();
    changed.callables[0].checked_crash.inferred = PackagePolicyInferredCrash::Complete {
        causes: vec![
            PackageReviewCrashCause::Trap,
            PackageReviewCrashCause::Abort,
        ],
    };
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0]
        .checked_crash
        .structural_runtime_requirements = None;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0]
        .checked_crash
        .structural_runtime_requirements = Some(Vec::new());
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].mutation.completeness = PackageReviewWriteFrameCompleteness::Opaque;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_termination = PackagePolicyTermination::NoGuarantee;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].declared_may_suspend = Some(true);
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].declared_may_block = Some(true);
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].declared_termination =
        Some(changed.callables[0].checked_termination.clone());
    cases.push(changed);
    for changed in cases {
        let changed_bytes = changed.canonical_bytes().unwrap();
        assert_ne!(changed_bytes, bytes);
        assert_eq!(recover(&changed_bytes).unwrap(), changed);
    }
}

#[test]
fn policy_rejects_detached_roles_owners_coordinates_and_noncanonical_sets() {
    let original = fixture();
    let mut cases = Vec::new();
    let mut changed = original.clone();
    changed.callables.push(changed.callables[0].clone());
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].identity.owner = PackageReviewNominalOwner::Package(
        psi_core::PackageKeyIdentity::from_digest([18; 32]).unwrap(),
    );
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].role = PackagePolicyCallableRole::PrivateAssumption;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].role = PackagePolicyCallableRole::PrivateExternal;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].role = PackagePolicyCallableRole::Boundary;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].supply = PackageReviewCallableSupply::AdmissionClaim;
    changed.callables[0].checked_service_reach = PackageReviewCheckedServiceReach::NoCheckedBody;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].declared_service_reach = None;
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].capability_flows.reverse();
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].reachable_capability_flows.reverse();
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].reachable_capability_flows.pop();
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_crash.inferred =
        PackagePolicyInferredCrash::Complete { causes: Vec::new() };
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_crash.published.clear();
    changed.callables[0].checked_crash.inferred = PackagePolicyInferredCrash::Complete {
        causes: vec![PackageReviewCrashCause::Trap, PackageReviewCrashCause::Trap],
    };
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].conformances[0].trait_lifetime_arguments = vec![2];
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].conformances[0].requirement_lifetime_partition = vec![1];
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0].checked_crash.published[0]
        .alternative_guards
        .push(PackagePolicyCrashGuard::Truth);
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0]
        .checked_crash
        .structural_runtime_requirements =
        Some(vec![PackageReviewBooleanExpression::Local { position: 0 }]);
    cases.push(changed);
    let mut changed = original.clone();
    changed.callables[0]
        .checked_crash
        .structural_runtime_requirements = Some(vec![PackageReviewBooleanExpression::Parameter {
        position: 2,
    }]);
    cases.push(changed);
    let mut changed = original.clone();
    let repeated = changed.callables[0].mutation.paths[0].clone();
    changed.callables[0].mutation.paths.push(repeated);
    cases.push(changed);
    for policy in cases {
        assert!(policy.canonical_bytes().is_err(), "{policy:?}");
        assert!(
            recover(&unchecked_bytes(&policy)).is_err(),
            "malformed recovery {policy:?}"
        );
    }
}

#[test]
fn every_prefix_version_and_trailing_field_rejects() {
    let bytes = fixture().canonical_bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut changed = bytes.clone();
    changed[CALLABLE_POLICY_MAGIC.len()] = 255;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(recover(&changed), Err(Error::TrailingBytes));
}

#[test]
fn unknown_callable_roles_and_impossible_top_level_counts_reject() {
    let policy = fixture();
    let bytes = policy.canonical_bytes().unwrap();
    let count_position =
        CALLABLE_POLICY_MAGIC.len() + 2 + 32 + 8 + policy.target.identity().as_str().len();
    let mut changed = bytes.clone();
    changed[count_position + 8] = 255;
    assert_eq!(recover(&changed), Err(Error::InvalidTag));
    let mut changed = bytes.clone();
    changed[count_position + 9] = 255;
    assert_eq!(recover(&changed), Err(Error::InvalidIdentity));
    let mut changed = bytes;
    changed[count_position..count_position + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(recover(&changed), Err(Error::ElementLimitExceeded));
}
