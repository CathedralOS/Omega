//! Source-produced exclusion verdicts survive ordinary native publication.
//! Public allowances are not selected behavior: a no-op assertion avoids Trap,
//! and an ordinary silent logger avoids Sink. A silent boundary provider still
//! receives a Sink invocation. Test-only review policy admits these isolated
//! fixture closures; it does not grant authority to user projects.

use crate::accepted_policy_fixture::accepted_policy;
use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessLimits,
    BoundedProcessPrepared, run_bounded_process,
};
use package_manager::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, accept_ordinary_closure_evidence,
    realize_accepted_native_report,
};
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, resolve_workspace_project_closure,
};
use package_manager::review::{
    CanonicalPackageReconstructionQuestionLimits, ReviewOnlyCapabilityConflictLimits,
    SemanticBindingReview, compile_resolved_package_candidate_for_production,
};
use package_source::{
    LocalSourceLimits, PrimaryGitChoices, SourceLineage, SourceRelativePath, SourceResolver,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use terminal_psi_to_abstract_operations::provider_admission;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-behavior-exclusion-verdicts-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .join("tests/omega/packages/behavior-exclusions")
        .canonicalize()
        .expect("committed behavior-exclusions fixture exists")
}

/// The apps depend on sibling kits by relative path, so the whole committed
/// tree is copied rather than one project.
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Keep refusal diagnostics separate from publication and execution failures.
fn realize(
    workspace: &Path,
    app: &str,
    profile: target::TargetProfile,
) -> Result<compiler::CompileReport, String> {
    let storage =
        SourceResolver::for_hardened_base(workspace.join("cache"), PrimaryGitChoices::default())
            .unwrap();
    let closure = resolve_workspace_project_closure(
        &SourceLineage::git("https://example.com/behavior-exclusions-fixture.git").unwrap(),
        SourceRelativePath::parse(app).unwrap(),
        workspace,
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
    )
    .expect("fixture closure resolves");

    let target = closure.for_exact_target(profile);
    let candidate = compile_resolved_package_candidate_for_production(
        &target,
        &workspace.join("review").join(app),
        SemanticBindingReview::Explicit(&[]),
        None,
    )
    .map_err(|error| format!("{error:?}"))?;

    let policy = accepted_policy(&target, candidate.reviews());
    let evidence = accept_ordinary_closure_evidence(
        &target,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        Some(&policy),
    )
    .unwrap();
    realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback: &compiler::OptimizationRollback::default(),
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy: provider_admission::current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
            imports: &[],
        },
    )
    .map_err(|diagnostics| format!("{diagnostics:?}"))
}

/// Change only optional selection, before resolving the source closure.
fn select_optimization(workspace: &Path, app: &str, enabled: bool) {
    let path = workspace.join(app).join("build.omg");
    let source = fs::read_to_string(&path).unwrap();
    let selection = "    Optimizations::enable(&mut builder.optimizations, Optimization::ControlFlowCleanup);\n";
    let source = source.replace(selection, "");
    let source = if enabled {
        source.replacen(
            "machine build(builder: &mut Build) {\n",
            &format!("machine build(builder: &mut Build) {{\n{selection}"),
            1,
        )
    } else {
        source
    };
    fs::write(path, source).unwrap();
}

fn publish_and_run(workspace: &Path, app: &str, report: compiler::CompileReport) {
    let report = compilation_products::publication::publish_compilation(
        report,
        &workspace.join("published").join(app),
    )
    .expect("admitted product publishes");
    let executable = report
        .checked_native_executable_path()
        .expect("published native executable");
    let command = BoundedProcessPrepared::new(
        Command::new(executable),
        BoundedProcessLimits::new(
            10,
            1024 * 1024 * 1024,
            1024 * 1024,
            64,
            1,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
        ),
        "behavior exclusion fixture",
    )
    .expect("prepare native execution");
    let output = run_bounded_process(
        command,
        BoundedProcessInput::Null,
        BoundedCaptureLimits::new(
            4096,
            4096,
            Duration::from_secs(10),
            Duration::from_secs(2),
            Duration::from_millis(5),
        ),
        BoundedCaptureBudget::new(8192),
    )
    .expect("native execution completes within its budget");
    assert!(output.status.success(), "{app}: {:?}", output.status);
    assert!(output.stdout.is_empty(), "{app}: {:?}", output.stdout);
    assert!(output.stderr.is_empty(), "{app}: {:?}", output.stderr);
}

#[test]
fn compositions_avoiding_excluded_behavior_publish_and_run() {
    let Some(profile) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: native exclusion execution has no supported host target");
        return;
    };
    if profile == target::TargetProfile::MacosX64 {
        eprintln!("SKIP: macOS x86-64 is outside the hosted execution matrix");
        return;
    }
    for app in ["no-op-app", "quiet-logger-app"] {
        for optimized in [false, true] {
            let fixture = Fixture::new();
            copy_tree(&fixture_root(), &fixture.0);
            select_optimization(&fixture.0, app, optimized);
            let report = realize(&fixture.0, app, profile)
                .unwrap_or_else(|error| panic!("{app}, optimized={optimized}: {error}"));
            publish_and_run(&fixture.0, app, report);
        }
    }
}

/// The same build declaration over a provider that does reach `Sink::emit`
/// refuses, and names the service, the machine and the reaching call rather
/// than reporting an unsatisfied count alone.
#[test]
fn a_composition_reaching_the_excluded_service_refuses() {
    for optimized in [false, true] {
        let fixture = Fixture::new();
        copy_tree(&fixture_root(), &fixture.0);
        select_optimization(&fixture.0, "sink-app", optimized);
        expect_refusal(
            &fixture.0,
            "sink-app",
            &[
                "behavior exclusion violated",
                "service `Sink` is reachable",
                "Main::main",
            ],
        );
    }
}

fn expect_refusal(workspace: &Path, app: &str, expected: &[&str]) {
    let error = match realize(workspace, app, target::TargetProfile::LinuxX64) {
        Ok(_) => panic!("excluded behavior refuses before publication"),
        Err(error) => error,
    };
    for fragment in expected {
        assert!(
            error.contains(fragment),
            "{app}: refusal should name {fragment}: {error}"
        );
    }
}

#[test]
fn checking_assertions_refuse_with_or_without_optimization() {
    for optimized in [false, true] {
        let fixture = Fixture::new();
        copy_tree(&fixture_root(), &fixture.0);
        select_optimization(&fixture.0, "checking-app", optimized);
        expect_refusal(
            &fixture.0,
            "checking-app",
            &[
                "behavior exclusion violated",
                "crash cause Trap",
                "CheckingAssert::check",
            ],
        );
    }
}

#[test]
fn direct_unit_assertions_refuse_with_or_without_optimization() {
    for optimized in [false, true] {
        let fixture = Fixture::new();
        copy_tree(&fixture_root(), &fixture.0);
        let kit = fixture.0.join("assert-kit/main.omg");
        let mut source = fs::read_to_string(&kit).unwrap();
        source.push_str(&fs::read_to_string(fixture.0.join("direct-unit.omg")).unwrap());
        fs::write(kit, source).unwrap();
        let build = fixture.0.join("checking-app/build.omg");
        let source = fs::read_to_string(&build).unwrap().replace(
            "assert_kit::main::CheckingAssert",
            "assert_kit::main::DirectCheckingAssert",
        );
        fs::write(build, source).unwrap();
        select_optimization(&fixture.0, "checking-app", optimized);
        expect_refusal(
            &fixture.0,
            "checking-app",
            &[
                "behavior exclusion violated",
                "crash cause Trap",
                "DirectCheckingAssert::check",
            ],
        );
    }
}

#[test]
fn no_op_selection_does_not_erase_eager_argument_traps() {
    for optimized in [false, true] {
        let fixture = Fixture::new();
        copy_tree(&fixture_root(), &fixture.0);
        let main = fixture.0.join("no-op-app/main.omg");
        let source = fs::read_to_string(&main)
            .unwrap()
            .replace("1 == 1", "predicate()");
        fs::write(
            main,
            format!("{source}\nmachine predicate() -> bool crashes Trap {{ crash Trap; }}\n"),
        )
        .unwrap();
        select_optimization(&fixture.0, "no-op-app", optimized);
        expect_refusal(
            &fixture.0,
            "no-op-app",
            &[
                "behavior exclusion violated",
                "crash cause Trap",
                "predicate",
            ],
        );
    }
}

#[test]
fn conditional_helper_records_only_executed_value_call_selections() {
    for enabled in [false, true] {
        let fixture = Fixture::new();
        copy_tree(&fixture_root(), &fixture.0);
        let build = fixture.0.join("checking-app/build.omg");
        let source = fs::read_to_string(&build).unwrap().replace(
            "Build::exclude_crash(builder, CrashCause::Trap);",
            &format!("configure(builder, {enabled});"),
        );
        fs::write(
            build,
            format!(
                r#"{source}
machine configure(builder: &mut Build, enabled: bool) {{
    transition enabled {{ true -> selected(builder) false -> omitted() }}
    state selected(builder: &mut Build) {{
        let cause: CrashCause = CrashCause::Trap;
        _ = Build::exclude_crash(builder, cause);
    }}
    state omitted() {{ }}
}}
"#
            ),
        )
        .unwrap();
        if enabled {
            expect_refusal(
                &fixture.0,
                "checking-app",
                &["behavior exclusion violated", "crash cause Trap"],
            );
        } else {
            realize(&fixture.0, "checking-app", target::TargetProfile::LinuxX64)
                .unwrap_or_else(|error| panic!("an unexecuted exclusion selects nothing: {error}"));
        }
    }
}

#[test]
fn exclusion_selection_cannot_duplicate_the_build_activation() {
    let fixture = Fixture::new();
    copy_tree(&fixture_root(), &fixture.0);
    let build = fixture.0.join("no-op-app/build.omg");
    let source = fs::read_to_string(&build).unwrap().replace(
        "Build::exclude_crash(builder, CrashCause::Trap);",
        "let mut other: Build;\n    Build::exclude_physical_authority(&mut other, PhysicalAuthorityClass::ProcessTermination);",
    );
    fs::write(build, source).unwrap();
    expect_refusal(
        &fixture.0,
        "no-op-app",
        &["compiler-owned Build activation cannot be copied"],
    );
}

#[test]
fn a_same_named_user_machine_does_not_select_an_exclusion() {
    let fixture = Fixture::new();
    copy_tree(&fixture_root(), &fixture.0);
    let build = fixture.0.join("checking-app/build.omg");
    let source = fs::read_to_string(&build)
        .unwrap()
        .replace("Build::exclude_crash", "Other::exclude_crash");
    fs::write(build, format!("{source}\ndata Other {{}}\nmachine Other::exclude_crash(builder: &mut Build, cause: CrashCause) {{}}\n")).unwrap();
    realize(&fixture.0, "checking-app", target::TargetProfile::LinuxX64).unwrap_or_else(|error| {
        panic!("a user declaration cannot select a compiler operation: {error}")
    });
}
