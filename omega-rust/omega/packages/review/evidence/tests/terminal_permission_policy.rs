#[path = "terminal_permission_policy/calling.rs"]
mod calling;
#[path = "terminal_permission_policy/fixtures.rs"]
mod fixtures;
#[path = "terminal_permission_policy/generics.rs"]
mod generics;
mod support;
#[path = "terminal_permission_policy/uefi.rs"]
mod uefi;

use omega_effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use omega_package_evidence::encoding::PackagePolicyRecoveryLimits;
use omega_package_evidence::project_checked_terminal_permission_policy;
use omega_package_evidence::record::*;
use omega_target::TargetProfile;
use support::*;

fn project(
    checked: &CheckedCompilation,
    target: TargetProfile,
) -> PackagePolicyTerminalPermissions {
    let policy = project_checked_terminal_permission_policy(checked, target, package_identity())
        .expect("exact checked terminal permission policy");
    assert_eq!(policy.package(), package_identity());
    assert_eq!(policy.target(), target);
    let bytes = policy
        .canonical_bytes()
        .expect("canonical terminal permission policy");
    let recovered = PackagePolicyTerminalPermissions::recover_canonical(
        &bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("bounded terminal permission policy recovery");
    assert_eq!(policy, recovered);
    assert_eq!(bytes, recovered.canonical_bytes().unwrap());
    policy
}

fn read_permission() -> TerminalAuthorityDisposition {
    TerminalAuthorityDisposition::from_classes([TerminalAuthorityClass::FilesystemContentRead])
}

#[test]
fn explicit_empty_permission_is_distinct_from_absence_for_unused_requirement_only_service() {
    let fixture =
        fixtures::Fixture::filesystem(fixtures::FILESYSTEM, false, "FilesystemHost", "read");
    assert!(
        fixture
            .candidate
            .selected_provider_plans()
            .plans()
            .is_empty()
    );
    let absent_checked = fixture.check(None);
    let explicit_checked = fixture.check(Some(TerminalAuthorityDisposition::from_classes([])));
    assert!(
        explicit_checked
            .selected_provider_plans()
            .plans()
            .is_empty()
    );
    let absent = project(&absent_checked, fixture.target);
    let explicit = project(&explicit_checked, fixture.target);
    assert!(absent.services().is_empty());
    let [service] = explicit.services() else {
        panic!("explicit empty permission retains its service")
    };
    assert_eq!(service.service().path(), "FilesystemHost");
    assert_eq!(service.methods().len(), 2);
    let [permission] = service.permissions() else {
        panic!("one explicit permission row")
    };
    assert!(permission.permitted().classes().is_empty());
    assert_eq!(permission.requirement().path(), fixture.requirement);
    assert_ne!(absent, explicit);
    assert_ne!(
        absent.canonical_bytes().unwrap(),
        explicit.canonical_bytes().unwrap()
    );
}

#[test]
fn unpermitted_sibling_schema_changes_and_stale_checked_schema_are_not_hidden() {
    let original =
        fixtures::Fixture::filesystem(fixtures::FILESYSTEM, false, "FilesystemHost", "read");
    let changed_source = fixtures::FILESYSTEM.replace(
        "machine stat(descriptor: i32) -> i64;",
        "machine stat(descriptor: i32) -> i64 blocks;",
    );
    let changed = fixtures::Fixture::filesystem(&changed_source, false, "FilesystemHost", "read");
    let original_checked = original.check(Some(read_permission()));
    let original_policy = project(&original_checked, original.target);
    let changed_policy = project(&changed.check(Some(read_permission())), changed.target);
    assert_eq!(
        original_policy.services()[0].permissions(),
        changed_policy.services()[0].permissions()
    );
    assert_ne!(
        original_policy.services()[0].methods(),
        changed_policy.services()[0].methods()
    );
    assert_ne!(
        original_policy.canonical_bytes().unwrap(),
        changed_policy.canonical_bytes().unwrap()
    );
    assert!(
        changed
            .check_binding(original.binding(Some(read_permission())))
            .is_err()
    );
    let mut stale = original_checked.clone();
    let signatures = stale
        .traits()
        .iter()
        .find(|service| service.name.as_str() == "FilesystemHost")
        .unwrap()
        .machines;
    stale
        .typed
        .trait_machine_signatures
        .span_mut(signatures)
        .unwrap()
        .iter_mut()
        .find(|method| method.name.as_str() == "stat")
        .unwrap()
        .blocks = true;
    assert!(
        project_checked_terminal_permission_policy(&stale, original.target, package_identity())
            .is_err()
    );
}

#[test]
fn exact_service_owner_and_requested_target_and_package_are_checked() {
    let local =
        fixtures::Fixture::filesystem(fixtures::FILESYSTEM, false, "FilesystemHost", "read");
    let foreign =
        fixtures::Fixture::filesystem(fixtures::FILESYSTEM, true, "FilesystemHost", "read");
    let local_policy = project(&local.check(Some(read_permission())), local.target);
    let checked = foreign.check(Some(read_permission()));
    let foreign_policy = project(&checked, foreign.target);
    assert_eq!(
        local_policy.services()[0].service().path(),
        foreign_policy.services()[0].service().path()
    );
    assert_eq!(
        foreign_policy.services()[0].service().owner(),
        PackageReviewNominalOwner::Package(foreign.owner)
    );
    assert_eq!(
        foreign_policy.services()[0].permissions()[0]
            .requirement()
            .owner(),
        PackageReviewNominalOwner::Package(foreign.owner)
    );
    assert_ne!(local_policy, foreign_policy);
    assert!(
        project_checked_terminal_permission_policy(
            &checked,
            TargetProfile::WindowsX64,
            package_identity()
        )
        .is_err()
    );
    assert!(
        project_checked_terminal_permission_policy(
            &checked,
            foreign.target,
            PackageKeyIdentity::from_digest([99; 32]).unwrap()
        )
        .is_err()
    );
}

#[test]
fn accepted_console_dependency_retains_process_permission_without_exercising_it() {
    let fixture = fixtures::Fixture::console();
    let checked = fixture.check(Some(TerminalAuthorityDisposition::from_classes([
        TerminalAuthorityClass::ProcessTermination,
    ])));
    let policy = project(&checked, fixture.target);
    let [service] = policy.services() else {
        panic!("one accepted Console permission service")
    };
    assert_eq!(
        service.service().owner(),
        PackageReviewNominalOwner::Package(fixture.owner)
    );
    assert_eq!(service.service().path(), "Console");
    assert_eq!(service.methods().len(), 1);
    assert_eq!(
        service.permissions()[0].permitted().classes(),
        &[TerminalAuthorityClass::ProcessTermination]
    );
    assert_eq!(
        service.permissions()[0].requirement(),
        service.methods()[0].requirement()
    );
}
