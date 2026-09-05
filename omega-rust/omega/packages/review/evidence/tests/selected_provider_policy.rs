mod support;

#[path = "selected_provider_policy/authority.rs"]
mod authority;
#[path = "selected_provider_policy/evaluated.rs"]
mod evaluated;
#[path = "selected_provider_policy/families.rs"]
mod families;
#[path = "selected_provider_policy/fixtures.rs"]
mod fixtures;
#[path = "selected_provider_policy/inherited.rs"]
mod inherited;
#[path = "selected_provider_policy/installation.rs"]
mod installation;
#[path = "selected_provider_policy/signatures.rs"]
mod signatures;

use fixtures::Fixture;
use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::project_checked_selected_provider_policy;
use package_evidence::record::*;
use support::*;
use target::TargetProfile;

fn project(fixture: &Fixture) -> PackagePolicySelectedProviders {
    let policy = project_checked_selected_provider_policy(
        &fixture.checked,
        fixture.target,
        package_identity(),
    )
    .expect("exact source-derived selected provider policy");
    assert_eq!(policy.package(), package_identity());
    assert_eq!(policy.target(), fixture.target);
    let bytes = policy
        .canonical_bytes()
        .expect("complete provider policy encoding");
    let recovered = PackagePolicySelectedProviders::recover_canonical(
        &bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("bounded receipt-free provider policy recovery");
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    policy
}
