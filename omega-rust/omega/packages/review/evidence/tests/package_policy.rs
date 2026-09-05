//! Full inert baseline assembled from real checked package declarations.

#[path = "package_policy/components.rs"]
mod components;
#[path = "package_policy/contracts.rs"]
mod contracts;
#[path = "package_policy/public_families.rs"]
mod public_families;
#[path = "package_policy/source.rs"]
mod source;
mod support;

use package_evidence::encoding::{
    PackagePolicyMembershipLimits, PackagePolicyRecoveryLimits, PackagePolicyTextRecoveryLimits,
};
use package_evidence::record::*;
use package_evidence::{
    project_checked_callable_policy, project_checked_package_policy,
    project_checked_representation_policy, project_checked_selected_provider_policy,
    project_checked_terminal_permission_policy,
};
use source::Fixture;
use support::*;
use target::TargetProfile;

fn project(fixture: &Fixture) -> PackagePolicyBaseline {
    let policy =
        project_checked_package_policy(&fixture.checked, fixture.target, package_identity())
            .expect("complete checked package policy");
    assert_eq!(policy.package(), package_identity());
    assert_eq!(policy.target(), fixture.target);
    let membership = policy
        .validate_package_membership(|_| true, PackagePolicyMembershipLimits::default())
        .expect("all compiler-issued semantic identity families have supported owner grammar");
    let bytes = policy.canonical_bytes().expect("canonical full baseline");
    let recovered =
        PackagePolicyBaseline::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .expect("recover complete baseline without compiler state or source reads");
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    let text = policy
        .canonical_text()
        .expect("complete named baseline text");
    let text_policy =
        PackagePolicyBaseline::recover_text(&text, PackagePolicyTextRecoveryLimits::default())
            .expect("recover named baseline without source");
    assert_eq!(text_policy, policy);
    assert_eq!(text_policy.canonical_bytes().unwrap(), bytes);
    assert_eq!(text_policy.canonical_text().unwrap(), text);
    assert_eq!(
        text_policy
            .validate_package_membership(|_| true, PackagePolicyMembershipLimits::default())
            .unwrap(),
        membership,
    );
    policy
}

#[test]
fn assembly_retains_each_existing_component_and_checks_requested_scope() {
    let fixture = Fixture::local(public_families::ALL_FAMILIES);
    let policy = project(&fixture);
    assert_eq!(
        policy.callables(),
        &project_checked_callable_policy(&fixture.checked, fixture.target, package_identity())
            .unwrap()
    );
    assert_eq!(
        policy.selected_providers(),
        &project_checked_selected_provider_policy(
            &fixture.checked,
            fixture.target,
            package_identity()
        )
        .unwrap()
    );
    assert_eq!(
        policy.terminal_permissions(),
        &project_checked_terminal_permission_policy(
            &fixture.checked,
            fixture.target,
            package_identity()
        )
        .unwrap()
    );
    assert_eq!(
        policy.representation(),
        &project_checked_representation_policy(&fixture.checked, package_identity()).unwrap()
    );
    assert!(policy.external_supplies().is_empty());
    assert!(
        project_checked_package_policy(
            &fixture.checked,
            TargetProfile::LinuxX64,
            package_identity()
        )
        .is_err()
    );
    assert!(
        project_checked_package_policy(
            &fixture.checked,
            fixture.target,
            PackageKeyIdentity::from_digest([99; 32]).unwrap()
        )
        .is_err()
    );
}
