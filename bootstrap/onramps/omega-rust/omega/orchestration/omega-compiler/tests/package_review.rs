use omega_compiler::{
    PACKAGE_REVIEW_ENCODING_VERSION, PackageCompilationInputs, PackageReviewCallableRole,
    PackageReviewCrashInterface, PackageReviewCrashRouteGuard, PackageReviewNominalOwner,
    PackageReviewSynchronousInvocation, PackageSourceBinding, compile_to_checked_with_packages,
    project_checked_package_review,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-review-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review fixture");
        Self(path)
    }

    fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review fixture source");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity")
}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}

#[test]
fn review_projects_root_boundary_and_build_authority() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine host_ping() reaches <= Host;
boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via Binding::VtableSlot(1);
data Receipt [linear] { code: i32; }
machine helper()
crashes Abort
{
    crash Abort;
}
pub machine public_api() { }
machine private_api() { }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build)
crashes Abort
{
    helper();
    let receipt: Receipt = Receipt { code: 1 };
    crash Abort;
}
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("package fixture should check");
    let review = project_checked_package_review(&checked).expect("review projection should close");
    let encoded = review
        .canonical_review_bytes()
        .expect("review projection should have a canonical comparison encoding");
    let magic = b"OMEGA-PACKAGE-REVIEW\0";
    assert!(encoded.starts_with(magic));
    assert_eq!(
        &encoded[magic.len()..magic.len() + 2],
        &PACKAGE_REVIEW_ENCODING_VERSION.to_le_bytes(),
    );
    assert_eq!(
        encoded,
        review
            .canonical_review_bytes()
            .expect("repeated encoding must be deterministic")
    );

    assert_eq!(review.package(), package_identity());
    assert_eq!(
        review.target().target_name(),
        target,
        "review identity must retain the deployment profile, not only its native ABI",
    );
    assert_eq!(PACKAGE_REVIEW_ENCODING_VERSION, 2);
    assert_eq!(review.callables().len(), 3);
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary row");
    assert_eq!(boundary.identity().path(), "host_ping");
    assert_eq!(
        boundary.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let [declared] = boundary
        .declared_service_reach()
        .expect("installation-bound declaration retains its upper bound")
    else {
        panic!("one declared upper-bound service")
    };
    assert_eq!(declared.path(), "Host");
    assert_eq!(
        declared.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        boundary.realized_service_reach(),
        boundary
            .declared_service_reach()
            .expect("published upper bound")
    );
    assert!(boundary.concrete_service_reach().is_empty());
    assert!(boundary.capability_flows().is_empty());
    assert_eq!(boundary.declared_synchronous_invocations(), Some(&[][..]));
    assert!(boundary.realized_synchronous_invocations().is_empty());
    let [installation] = boundary.unresolved_installation_reaches() else {
        panic!("one normalized installation-bound reach row")
    };
    assert_eq!(
        installation.requirement().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(installation.requirement().path().contains("host_ping"));
    let [upper_bound] = installation.upper_bound() else {
        panic!("one normalized installation upper-bound service")
    };
    assert_eq!(
        upper_bound.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(upper_bound.path(), "Host");

    let build = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Build)
        .expect("build row");
    assert_eq!(build.identity().path(), "build");
    assert_eq!(build.declared_service_reach(), None);
    assert_eq!(build.declared_synchronous_invocations(), None);

    let public = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("ordinary public callable row");
    assert_eq!(public.identity().path(), "public_api");
    assert_eq!(
        public.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(public.declared_service_reach(), Some(&[][..]));
    assert_eq!(public.declared_synchronous_invocations(), Some(&[][..]));
    assert!(public.realized_service_reach().is_empty());
    assert!(public.realized_synchronous_invocations().is_empty());
    assert_eq!(
        public.checked_crash().interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "private_api")
    );
    let crash = build.checked_crash();
    assert_eq!(
        crash.interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    let [published_crash] = crash.published() else {
        panic!("one normalized published crash route")
    };
    assert_eq!(
        published_crash.cause(),
        psi_checked_trees::CrashCause::Abort
    );
    assert_eq!(
        published_crash.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let [crash_site] = crash.checked_sites() else {
        panic!("one normalized checked crash site")
    };
    assert_eq!(
        crash_site.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_site.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(crash_site.guard_covering_buckets(), [1]);
    assert!(!crash_site.frontier_lower_bound().is_empty());
    assert!(
        crash_site
            .frontier_lower_bound()
            .iter()
            .all(|claim| claim.machine().owner()
                == PackageReviewNominalOwner::Package(package_identity())
                && claim.state().owner() == PackageReviewNominalOwner::Package(package_identity()))
    );
    let [crash_call] = crash.checked_calls() else {
        panic!("one normalized checked crash call")
    };
    assert_eq!(
        crash_call.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        crash_call.target_machine().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_call.target_machine().path(), "helper");
    assert_eq!(
        crash_call.target_state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let [provider] = review.selected_providers() else {
        panic!("one selected provider review row")
    };
    assert_eq!(provider.realizing_package(), Some(package_identity()));
    assert_eq!(provider.provider_type_package(), None);
    assert_eq!(provider.service_schema(), "Host");
    assert_eq!(
        provider.schema().trait_package_identity,
        Some(package_identity())
    );
    assert_eq!(
        provider.schema().methods[0].requirement_owner_package_identity,
        Some(package_identity())
    );
    assert_eq!(provider.rows().len(), 1);
    assert!(matches!(
        provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::VtableSlot { index: 1 }
    ));
}

#[test]
fn public_machine_visibility_survives_checked_compilation_and_strict_empty_contracts() {
    let package = TempPackage::new();
    package.write("main.omg", "pub machine Package::entry() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public machine should check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("checked public machine");
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );

    let service = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("checked service contract");
    assert!(matches!(
        service.interface,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(_)
    ));
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("checked invocation contract");
    assert_eq!(
        invocation.interface,
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling
    );
    assert!(matches!(
        checked
            .facts
            .suspensions
            .for_machine(machine.symbol)
            .expect("checked suspension contract")
            .interface,
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(false)
    ));
    assert!(matches!(
        checked
            .facts
            .blocking
            .for_machine(machine.symbol)
            .expect("checked blocking contract")
            .interface,
        psi_language_semantics::BlockingInterface::PublishedMayBlock(false)
    ));
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("checked contract")
            .crash
            .interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Handler) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Waiting)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            None,
            package_inputs(&package.0),
        )
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_v2_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"boundary trait Handler { machine handle(); }
boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
reaches Host
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"boundary trait Handler { machine handle(); }
boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(
        quiet_dispatch.contract_fingerprint(),
        dispatch.contract_fingerprint()
    );
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        None,
        package_inputs(&package.0),
    )
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = omega_compiler::compile_to_checked(&package.0.join("main.omg"), None)
        .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_distinguishes_profiles_that_share_a_native_target() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target uefi_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let windows = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("Windows review fixture should check");
    let uefi = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("uefi_x64"),
        package_inputs(&package.0),
    )
    .expect("UEFI review fixture should check");

    assert_eq!(
        windows.selected_native_target(),
        uefi.selected_native_target()
    );
    let windows = project_checked_package_review(&windows).expect("Windows review projection");
    let uefi = project_checked_package_review(&uefi).expect("UEFI review projection");
    assert_eq!(windows.target(), omega_target::TargetProfile::WindowsX64);
    assert_eq!(uefi.target(), omega_target::TargetProfile::UefiX64);
    assert_ne!(windows.target(), uefi.target());
    assert_ne!(
        windows.canonical_review_bytes().expect("Windows encoding"),
        uefi.canonical_review_bytes().expect("UEFI encoding"),
    );
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}

#[test]
fn review_rejects_contract_entailment_stand_downs() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    a >= 1
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary checking should retain the out-of-language stand-down");
    let [stand_down] = checked.contract_entailment_stand_downs() else {
        panic!("one exact contract-entailment stand-down")
    };
    assert_eq!(stand_down.contract_index, 1);
    assert_eq!(stand_down.fact_index, 0);
    assert_eq!(
        stand_down.reason,
        psi_validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage
    );

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("package review must fail closed on an unresolved stand-down");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects unresolved contract-entailment stand-down")
    }));
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
