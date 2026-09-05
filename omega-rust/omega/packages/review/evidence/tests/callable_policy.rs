//! Receipt-free callable meaning derived from actual checked source.

#[path = "callable_policy/crash.rs"]
mod crash;
#[path = "callable_policy/fixtures.rs"]
mod fixtures;
#[path = "callable_policy/flows.rs"]
mod flows;
#[path = "callable_policy/mutation.rs"]
mod mutation;
#[path = "callable_policy/progress.rs"]
mod progress;
#[path = "callable_policy/signatures.rs"]
mod signatures;
mod support;

use fixtures::Fixture;
use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::project_checked_callable_policy;
use package_evidence::record::*;
use support::*;
use target::TargetProfile;

fn project(fixture: &Fixture) -> PackagePolicyCallables {
    let policy =
        project_checked_callable_policy(&fixture.checked, fixture.target, package_identity())
            .expect("exact checked callable policy");
    assert_eq!(policy.package(), package_identity());
    assert_eq!(policy.target(), fixture.target);
    let bytes = policy.canonical_bytes().expect("canonical callable policy");
    let recovered =
        PackagePolicyCallables::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .expect("bounded callable policy recovery");
    assert_eq!(policy, recovered);
    assert_eq!(bytes, recovered.canonical_bytes().unwrap());
    policy
}

fn callable<'policy>(
    policy: &'policy PackagePolicyCallables,
    name: &str,
) -> &'policy PackagePolicyCallable {
    let matches = policy
        .callables()
        .iter()
        .filter(|callable| callable_has_name(callable, name))
        .collect::<Vec<_>>();
    let [callable] = matches.as_slice() else {
        panic!("expected one callable named {name}: {policy:#?}")
    };
    callable
}

fn callable_has_name(callable: &PackagePolicyCallable, name: &str) -> bool {
    // Select the leading authored machine-name field for test navigation;
    // equality assertions still compare the complete typed overload identity.
    let label = "conformance-callable";
    callable
        .identity()
        .path()
        .starts_with(&format!("{}:{label}{}:{name}", label.len(), name.len()))
}

#[test]
fn exact_requested_root_target_and_live_authored_reach_are_rejoined() {
    let fixture = Fixture::local(
        r#"
pub boundary trait Host { machine ping() reaches Host; }
pub boundary trait Other {}
pub machine dispatch() reaches Host invokes Host; { Host::ping(); }
"#,
    );
    project(&fixture);
    assert!(
        project_checked_callable_policy(
            &fixture.checked,
            TargetProfile::LinuxX64,
            package_identity()
        )
        .is_err()
    );
    assert!(
        project_checked_callable_policy(
            &fixture.checked,
            fixture.target,
            PackageKeyIdentity::from_digest([99; 32]).unwrap()
        )
        .is_err()
    );
    let mut changed = fixture.checked.clone();
    let dispatch = changed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "dispatch")
        .unwrap()
        .symbol;
    let other = changed
        .traits()
        .iter()
        .find(|service| service.name.as_str() == "Other")
        .unwrap()
        .symbol;
    changed
        .typed
        .authored_service_reach_rows
        .iter_mut()
        .find(|row| row.owner == dispatch)
        .unwrap()
        .targets[0]
        .service = other;
    assert!(project_checked_callable_policy(&changed, fixture.target, package_identity()).is_err());
}
