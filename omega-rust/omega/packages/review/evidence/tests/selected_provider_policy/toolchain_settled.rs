//! The toolchain-settled `FilesystemHost` plan replays its exact minted
//! identity through provider-policy review instead of failing as an authored
//! candidate, while ordinary authored plans in the same custody still replay.

use crate::support::*;
use compiler::CheckedCompileRequest;
use package_compilation::AcceptedSemanticBindingRole;
use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::project_checked_selected_provider_policy;
use package_evidence::record::*;
use target::TargetProfile;

const SOURCE: &str = r#"pub boundary trait FilesystemHost {
    machine close(fd: i32) -> i32 reaches FilesystemHost;
    machine find_close(handle: i64) -> i32 reaches FilesystemHost;
    machine close_handle(handle: i64) -> i32 reaches FilesystemHost;
    machine seek(fd: i32, offset: i64, whence: i32) -> i64 reaches FilesystemHost;
    machine duplicate(fd: i32) -> i32 reaches FilesystemHost;
    machine lock_file(fd: i32, operation: i32) -> i32 reaches FilesystemHost;
    machine sync(fd: i32) -> i32 reaches FilesystemHost;
    machine sync_data(fd: i32) -> i32 reaches FilesystemHost;
    machine set_len(fd: i32, length: i64) -> i32 reaches FilesystemHost;
    machine set_file_permissions(fd: i32, mode: u32) -> i32 reaches FilesystemHost;
    machine change_file_owner(fd: i32, uid: i32, gid: i32) -> i32 reaches FilesystemHost;
}
pub boundary trait Console {
    machine exit_process(return_code: i32) reaches Console;
}
pub data ConsoleNativeProvider {}
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
pub machine expose() reaches Console + FilesystemHost {}
"#;

const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("review-fixture");
    builder.select_provider<Console, ConsoleNativeProvider>();
}
"#;

const TIME_SOURCE: &str = r#"pub boundary trait TimeHost {
    machine monotonic_ticks() -> u64 reaches TimeHost;
    machine monotonic_ticks_per_second() -> u64 reaches TimeHost;
    machine wall_clock_raw() -> u64 reaches TimeHost;
    machine wall_clock_units_per_second() -> u64 reaches TimeHost;
    machine wall_clock_epoch_offset_seconds() -> u64 reaches TimeHost;
    machine sleep(milliseconds: u32) reaches TimeHost;
}
pub boundary trait Console {
    machine exit_process(return_code: i32) reaches Console;
}
pub data ConsoleNativeProvider {}
linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
pub machine expose() reaches Console + TimeHost {}
"#;

fn checked_with_filesystem_binding() -> (TempPackage, ReviewFixture) {
    let package = TempPackage::new();
    package.write("main.omg", SOURCE);
    package.write("build.omg", BUILD);
    let candidate = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("exposed filesystem candidate checks");
    let filesystem = candidate
        .custody
        .candidate_service_binding(
            AcceptedSemanticBindingRole::FilesystemHostService,
            package_identity(),
            "FilesystemHost",
        )
        .expect("derive exact filesystem binding");
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(
            package_inputs(&package.0)
                .with_accepted_semantic_bindings(vec![filesystem])
                .expect("filesystem binding belongs to the fixture package"),
        ),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("accepted filesystem service identity checks");
    (package, checked)
}

fn checked_with_time_host_binding() -> (TempPackage, ReviewFixture) {
    let package = TempPackage::new();
    package.write("main.omg", TIME_SOURCE);
    package.write("build.omg", BUILD);
    let candidate = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("exposed time host candidate checks");
    let time_host = candidate
        .custody
        .candidate_service_binding(
            AcceptedSemanticBindingRole::TimeHostService,
            package_identity(),
            "TimeHost",
        )
        .expect("derive exact time host binding");
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(
            package_inputs(&package.0)
                .with_accepted_semantic_bindings(vec![time_host])
                .expect("time host binding belongs to the fixture package"),
        ),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("linux_x86_64"))
    })
    .expect("accepted time host service identity checks");
    (package, checked)
}

#[test]
fn toolchain_settled_plan_replays_its_exact_settled_identity() {
    let (_package, checked) = checked_with_filesystem_binding();
    let policy = project_checked_selected_provider_policy(
        &checked,
        TargetProfile::LinuxX64,
        package_identity(),
    )
    .expect("toolchain-settled plan validates its settled identity");
    let settled = policy
        .plans()
        .iter()
        .find(|plan| plan.provider_type_declaration().is_none())
        .expect("one plan carries no authored provider declaration");
    assert_eq!(settled.provider_type(), "omega::toolchain::filesystem_host");
    assert_eq!(settled.realizing_package(), None);
    assert_eq!(settled.target(), "linux_x86_64");
    assert_eq!(settled.rows().len(), 11);
    assert_eq!(settled.methods().len(), settled.rows().len());
    for row in settled.rows() {
        assert!(row.realization().is_none());
        assert!(row.requirement_lifetime_partition().is_empty());
        assert!(matches!(
            row.binding(),
            PackagePolicyProviderBinding::Syscall {
                evaluated: None,
                ..
            }
        ));
    }
    // The authored `ConsoleNativeProvider` plan in the same custody still
    // replays as an authored candidate with exact realization machines.
    let authored = policy
        .plans()
        .iter()
        .find(|plan| plan.provider_type_declaration().is_some())
        .expect("the authored plan still replays");
    assert_eq!(authored.realizing_package(), Some(package_identity()));
    assert!(
        authored
            .rows()
            .iter()
            .all(|row| row.realization().is_some())
    );
    // Canonical encoding retains the settled row shape through recovery.
    let bytes = policy.canonical_bytes().expect("policy encodes");
    let recovered = PackagePolicySelectedProviders::recover_canonical(
        &bytes,
        PackagePolicyRecoveryLimits::default(),
    )
    .expect("policy recovers");
    assert_eq!(recovered, policy);
}

#[test]
fn toolchain_settled_time_host_plan_replays_its_exact_settled_identity() {
    let (_package, checked) = checked_with_time_host_binding();
    let policy = project_checked_selected_provider_policy(
        &checked,
        TargetProfile::LinuxX64,
        package_identity(),
    )
    .expect("toolchain-settled time host plan validates its settled identity");
    let settled = policy
        .plans()
        .iter()
        .find(|plan| plan.provider_type_declaration().is_none())
        .expect("one plan carries no authored provider declaration");
    assert_eq!(settled.provider_type(), "omega::toolchain::time_host");
    assert_eq!(settled.realizing_package(), None);
    assert_eq!(settled.target(), "linux_x86_64");
    // Only the scalar-only requirements realizable by one positional kernel
    // syscall earn a row; the constant requirements stay uncovered.
    let mut rows = settled
        .rows()
        .iter()
        .map(|row| {
            assert!(row.realization().is_none());
            assert!(row.requirement_lifetime_partition().is_empty());
            match row.binding() {
                PackagePolicyProviderBinding::Syscall { number, evaluated } => {
                    assert_eq!(evaluated, &None);
                    (row.method().to_string(), *number)
                }
                other => panic!("time host row binds a syscall, not {other:?}"),
            }
        })
        .collect::<Vec<_>>();
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("monotonic_ticks".to_string(), 228),
            ("sleep".to_string(), 35),
            ("wall_clock_raw".to_string(), 228),
        ]
    );
    assert_eq!(settled.methods().len(), settled.rows().len());
}
