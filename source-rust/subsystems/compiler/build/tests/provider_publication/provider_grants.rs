//! Check the grant and callback contract a Build selects for a private service.
use super::{fixture_repository, run_project_build};
use target::TargetProfile;

#[test]
fn package_review_retains_the_build_grant_for_a_private_provider() {
    use review_evidence::record::{
        PackagePolicyBaseline, PackagePolicySelectedProviders,
        PackageReviewProviderGrantSelectorKind,
    };

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
    let plan = reviewed
        .policy
        .selected_providers
        .plans
        .iter()
        .find(|plan| plan.schema_declaration.path == "SelectedCombine")
        .expect("the Build selected a provider for the private service");
    assert_eq!(
        plan.provider_type_declaration.as_ref().unwrap().path,
        "HostedCombine"
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
        .find(|callable| callable.identity.path == "SelectedCombine::probe")
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
    let index = providers
        .plans
        .iter()
        .position(|p| p.schema_declaration.path == "SelectedCombine")
        .unwrap();
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
