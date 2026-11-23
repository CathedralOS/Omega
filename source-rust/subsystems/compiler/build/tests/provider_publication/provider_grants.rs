//! Check the grant and callback contract a Build selects for a private service.
use super::{fixture_repository, run_project_build};
use review_evidence::record::{
    PackagePolicyBaseline, PackagePolicySelectedProviders, PackageReviewProviderGrantSelectorKind,
};
use target::TargetProfile;

fn check_selected_plan_target(mut policy: PackagePolicyBaseline, index: usize) {
    assert_eq!(
        policy.selected_providers.plans[index].target,
        policy.target.target_name()
    );
    for target in [TargetProfile::WindowsX64.target_name(), ""] {
        target.clone_into(&mut policy.selected_providers.plans[index].target);
        let component = policy.selected_providers.canonical_bytes().unwrap();
        assert!(
            PackagePolicySelectedProviders::recover_canonical(&component, Default::default())
                .is_err(),
            "recovered selected provider with target {target:?}"
        );
        let baseline = policy.canonical_bytes().unwrap();
        assert!(
            PackagePolicyBaseline::recover_canonical(&baseline, Default::default()).is_err(),
            "recovered package baseline with selected provider target {target:?}"
        );
    }
}

#[test]
fn package_review_retains_the_build_grant_for_a_private_provider() {
    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("private-service", false)
            .unwrap();
    let root = temporary.path();
    let fixture =
        fixture_repository::root().join("samples/cli/facts/dynamic_trait_dispatch/target_body");
    for name in ["build.omg", "main.omg"] {
        std::fs::copy(fixture.join(name), root.join(name)).unwrap();
    }
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let reviewed = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    assert!(reviewed.policy.public_api.traits.is_empty());
    assert!(reviewed.policy.public_api.data.is_empty());
    let plans = &reviewed.policy.selected_providers.plans;
    let index = plans
        .iter()
        .position(|plan| plan.schema_declaration.path == "main.omg::SelectedCombine")
        .expect("the Build selected a provider for the private service");
    let plan = &plans[index];
    assert_eq!(
        plan.provider_type_declaration.as_ref().unwrap().path,
        "main.omg::HostedCombine"
    );
    assert_eq!(
        plan.grants,
        [PackageReviewProviderGrantSelectorKind::ProviderApplication(
            None
        )]
    );
    let [method] = plan.methods.as_slice() else {
        panic!("the service has one callback");
    };
    assert_eq!(
        method.authority.synchronous_invocations,
        [review_evidence::record::PackageReviewSynchronousInvocation::Parameter(0)]
    );
    let policy = reviewed.policy;
    let binary = policy.canonical_bytes().unwrap();
    check_selected_plan_target(policy, index);
    let mut recovered =
        PackagePolicyBaseline::recover_canonical(&binary, Default::default()).unwrap();
    assert_eq!(recovered.canonical_bytes().unwrap(), binary);
    let package = recovered.package;
    recovered
        .validate_package_membership(|key| key == package, Default::default())
        .unwrap();
    let callback = recovered
        .callables
        .callables
        .iter_mut()
        .find(|callable| callable.identity.path == "main.omg::SelectedCombine::probe")
        .unwrap();
    assert_eq!(callback.lifetime_parameter_count, 1);
    callback.lifetime_parameter_count = 0;
    let error = recovered
        .validate_package_membership(|key| key == package, Default::default())
        .unwrap_err();
    assert_eq!(
        error.error(),
        review_evidence::encoding::PackagePolicyMembershipError::MalformedIdentity
    );

    // Old locks name the slot or plan. They must still round-trip after a Build
    // can also name a particular application of a provider.
    let mut providers = recovered.selected_providers;
    for grant in [
        PackageReviewProviderGrantSelectorKind::ProviderSlot,
        PackageReviewProviderGrantSelectorKind::PlanName,
        PackageReviewProviderGrantSelectorKind::ProviderApplication(Some("Selected<u64>".into())),
    ] {
        providers.plans[index].grants = vec![grant];
        let mut bytes = providers.canonical_bytes().unwrap();
        assert_eq!(
            PackagePolicySelectedProviders::recover_canonical(&bytes, Default::default()).unwrap(),
            providers
        );
        // A new application must never be accepted as a version 27 grant, or vice versa.
        let header = b"OMEGA-SELECTED-PROVIDER-POLICY\0".len();
        bytes[header] = if bytes[header] == 27 { 28 } else { 27 };
        assert!(
            PackagePolicySelectedProviders::recover_canonical(&bytes, Default::default()).is_err()
        );
    }
}

#[test]
fn package_review_recovers_a_native_bindings_caller_chosen_result() {
    use review_evidence::record::{PackagePolicyBaseline, PackageReviewForeignLocator};

    let temporary =
        platform_custody::temporary_directory::TemporaryDirectory::create("native-errno", false)
            .unwrap();
    let root = temporary.path();
    let fixture = fixture_repository::root()
        .join("source-rust/application/tests/fixtures/providers/run/provider_errno_realpath");
    std::fs::copy(fixture.join("build.omg"), root.join("build.omg")).unwrap();
    let mut main = std::fs::read_to_string(fixture.join("main.omg")).unwrap();
    // The private import and this public signature must retain the same checked result.
    main.push_str("\npublic boundary trait PublicErrno { machine slot() -> @i32 reaches none crashes none; }\n");
    std::fs::write(root.join("main.omg"), main).unwrap();

    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let reviewed = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    let bytes = reviewed.policy.canonical_bytes().unwrap();
    let policy = PackagePolicyBaseline::recover_canonical(&bytes, Default::default()).unwrap();
    policy
        .validate_package_membership(|key| key == policy.package, Default::default())
        .unwrap();

    let public = policy
        .public_api
        .traits
        .iter()
        .find(|row| row.identity.path == "main.omg::PublicErrno")
        .unwrap();
    let [slot] = public.requirements.as_slice() else {
        panic!("expected PublicErrno::slot")
    };
    assert_eq!(slot.lifetime_parameter_count, 1);
    assert!(slot.parameters.is_empty());
    assert!(slot.returned_borrow_origins.is_empty());
    let result = slot.return_type.as_ref().unwrap();
    assert_eq!(
        result.canonical,
        r"ref(named(name(compiler-type\(atom\(i32\)\))),region(owner(schema),ordinal(0)))"
    );

    let binding = policy
        .native_bindings
        .iter()
        .find(|row| row.method.requirement.path == "main.omg::HostErrno::slot")
        .unwrap();
    assert!(
        matches!(binding.locator(), Some(PackageReviewForeignLocator::ElfVersioned { object, symbol, version })
        if object == b"libc.so.6" && symbol == b"__errno_location" && version == b"GLIBC_2.2.5")
    );
    let signature = &binding.method.signature;
    assert_eq!(signature.requirement_lifetime_parameter_count, 1);
    assert!(signature.parameters.is_empty());
    assert_eq!(signature.result.as_ref(), Some(result));
    let [calling] = binding.method.calling.as_slice() else {
        panic!("expected one Linux calling application")
    };
    assert_eq!(calling.requirement_lifetime_parameter_count, 1);
    assert_eq!(calling.semantic_result.as_ref(), Some(result));

    let callable = policy
        .callables
        .callables
        .iter()
        .find(|row| row.identity == binding.method.requirement)
        .unwrap();
    assert_eq!(callable.lifetime_parameter_count, 1);
    assert!(callable.parameters.is_empty());
    assert_eq!(callable.return_type.as_ref(), Some(result));
}
