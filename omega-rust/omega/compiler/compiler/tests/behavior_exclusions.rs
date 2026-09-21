//! Compiled-corpus acceptance for build behavior exclusions
//! (wiki/spec/build/behavior_exclusions.md): one assertion library whose
//! checking and no-op implementations share the same public Trap ceiling,
//! composed by two application packages through ordinary build provider
//! selection, driven through the compile path with package inputs.
//!
//! The fixture lives at `tests/fixtures/packages/behavior-exclusions`.
//! Under a Trap exclusion only the no-op composition passes, with the build's
//! optional optimization enabled and rolled back alike; the checking
//! composition is reported as prohibited possible behavior with the machine
//! that retains it, never as insufficient evidence.

use compiler::{
    CompileOptions, CompileRequest, OptimizationRollback, RequestedCompileProduct, compile,
};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_BUILD: AtomicU64 = AtomicU64::new(0);

const TARGET: &str = "linux_x86_64";
const OPTIMIZATION: &str = "ControlFlowCleanup";

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/fixtures/packages/behavior-exclusions")
}

fn identity(seed: u8) -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([seed; 32]).expect("nonzero package identity")
}

/// The application `app` composed with its one library dependency under the
/// alias its source imports (`assert_kit` / `logger_kit`).
fn inputs(root: &Path, app: &str) -> PackageCompilationInputs {
    let (library, alias) = match app {
        "quiet-logger-app" | "sink-app" => ("logger-kit", "logger_kit"),
        _ => ("assert-kit", "assert_kit"),
    };
    PackageCompilationInputs::new_package(
        identity(0x61),
        vec![
            PackageSourceBinding::new(identity(0x61), app, root.join(app)),
            PackageSourceBinding::new(identity(0x62), library, root.join(library)),
        ],
        vec![PackageDependencyBinding::new(
            identity(0x61),
            alias,
            identity(0x62),
        )],
    )
    .expect("the behavior-exclusions fixture graph validates")
}

fn build_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-behavior-exclusions-{label}-{}-{}",
        std::process::id(),
        NEXT_BUILD.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn request(
    app: &str,
    product: RequestedCompileProduct,
    rollback: OptimizationRollback,
    label: &str,
) -> CompileRequest {
    CompileRequest::new(CompileOptions {
        root_path: fixture_root().join(app).join("main.omg"),
        build_dir: Some(build_dir(label)),
        target_name: Some(TARGET.to_owned()),
    })
    .with_package_inputs(inputs(&fixture_root(), app))
    .with_requested_product(product)
    .with_optimization_rollback(rollback)
}

fn rolled_back() -> OptimizationRollback {
    OptimizationRollback::from_exact_names([OPTIMIZATION])
        .expect("the fixture enables ControlFlowCleanup, so it can be rolled back")
}

fn compile_one(
    app: &str,
    product: RequestedCompileProduct,
    rollback: OptimizationRollback,
    label: &str,
) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
    compile(request(app, product, rollback, label))
        .and_then(compiler::CompileOutcomes::into_single_report)
}

fn messages(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A private copy lets tests vary the selected target/provider without
/// mutating shared fixtures or changing the library's public crash ceiling.
struct AssertionProject(PathBuf);

impl AssertionProject {
    fn new(target: &str, direct: bool) -> Self {
        let directory = build_dir("assertion-project");
        for package in ["assert-kit", "checking-app", "no-op-app"] {
            std::fs::create_dir_all(directory.join(package)).unwrap();
            for file in ["main.omg", "build.omg"] {
                let original =
                    std::fs::read_to_string(fixture_root().join(package).join(file)).unwrap();
                let source = if file == "build.omg" {
                    let source = original.replace(
                        "linux_x86_64::ProgramEntry",
                        &format!("{target}::ProgramEntry"),
                    );
                    if direct {
                        source.replace("::CheckingAssert>", "::DirectCheckingAssert>")
                    } else {
                        source
                    }
                } else {
                    original
                };
                std::fs::write(directory.join(package).join(file), source).unwrap();
            }
        }
        if direct {
            let library = directory.join("assert-kit/main.omg");
            let source = std::fs::read_to_string(&library).unwrap()
                + &std::fs::read_to_string(fixture_root().join("direct-unit.omg")).unwrap();
            std::fs::write(library, source).unwrap();
        }
        Self(directory)
    }

    fn compile(
        &self,
        app: &str,
        target: &str,
        rollback: OptimizationRollback,
    ) -> Result<compiler::CompileReport, Vec<diagnostics::Diagnostic>> {
        compile(
            CompileRequest::new(CompileOptions {
                root_path: self.0.join(app).join("main.omg"),
                build_dir: None,
                target_name: Some(target.to_owned()),
            })
            .with_package_inputs(inputs(&self.0, app))
            .with_optimization_rollback(rollback)
            .with_requested_product(RequestedCompileProduct::NativeArtifact),
        )
        .and_then(compiler::CompileOutcomes::into_single_report)
    }
}

impl Drop for AssertionProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_direct_unit_plan_does_not_establish_absence() {
    let project = AssertionProject::new(TARGET, true);
    let Err(diagnostics) = project.compile("checking-app", TARGET, OptimizationRollback::default())
    else {
        panic!("a missing Unit plan cannot prove absence of Trap");
    };
    let text = messages(&diagnostics);
    // This is a fail-closed frontier, not the completed exclusion verdict.
    // BUILD-SEMANTIC-EXCLUSIONS retains the required direct-Unit acceptance.
    assert!(
        text.contains("InvalidUnitMachinePlan")
            && text.contains("attached Unit closure is missing a checked transitive machine plan"),
        "{text}"
    );
    assert!(text.contains("DirectCheckingAssert::check"), "{text}");
}

#[test]
fn no_op_assertion_package_executes_on_the_host() {
    let Some(target) = target::TargetProfile::host_if_supported() else {
        eprintln!("SKIP: assertion package execution requires a supported host");
        return;
    };
    let project = AssertionProject::new(target.target_name(), false);
    // A false assertion must still return with the no-op provider. Keep the
    // original application fixture as the independent true-input control.
    let main = project.0.join("no-op-app/main.omg");
    let source = std::fs::read_to_string(&main)
        .unwrap()
        .replace("1 == 1", "1 == 0");
    std::fs::write(main, source).unwrap();
    for rollback in [OptimizationRollback::default(), rolled_back()] {
        let report = project
            .compile("no-op-app", target.target_name(), rollback)
            .unwrap_or_else(|diagnostics| {
                panic!("host native compilation: {}", messages(&diagnostics))
            });
        let published = report
            .publish_retained_native_artifact(&project.0.join("out"))
            .expect("publish host assertion package");
        let output = std::process::Command::new(
            published
                .checked_native_executable_path()
                .expect("checked executable"),
        )
        .output()
        .expect("execute host assertion package");
        assert!(output.status.success(), "{output:?}");
    }
}

#[test]
fn no_op_composition_satisfies_the_trap_exclusion_with_and_without_optimization() {
    for (label, rollback) in [
        ("no-op-optimized", OptimizationRollback::default()),
        ("no-op-rolled-back", rolled_back()),
    ] {
        let report = compile_one(
            "no-op-app",
            RequestedCompileProduct::TerminalArtifact,
            rollback,
            label,
        )
        .unwrap_or_else(|diagnostics| {
            panic!("{label}: the no-op composition satisfies the Trap exclusion: {diagnostics:#?}")
        });
        let retained = report
            .into_retained_terminal_artifact()
            .expect("retained Terminal product");
        let proposal = retained
            .native_realization_proposal()
            .expect("retained native proposal");
        assert!(
            proposal
                .behavior_exclusions()
                .excludes_crash_cause(terminal_psi::CrashCause::Trap),
            "{label}: the retained proposal carries the Trap exclusion"
        );
    }
}

#[test]
fn checking_composition_is_prohibited_under_the_same_public_ceiling() {
    for (label, rollback) in [
        ("checking-optimized", OptimizationRollback::default()),
        ("checking-rolled-back", rolled_back()),
    ] {
        let diagnostics = compile_one(
            "checking-app",
            RequestedCompileProduct::TerminalArtifact,
            rollback,
            label,
        )
        .expect_err("the checking composition retains a possible Trap");
        let text = messages(&diagnostics);
        assert!(
            text.contains("behavior exclusion violated: crash cause Trap"),
            "{label}: prohibited possible behavior must be reported: {text}"
        );
        assert!(
            text.contains("crash terminator"),
            "{label}: the site is the retained crash terminator: {text}"
        );
        // The rejection attributes the possible Trap to the declaration
        // that contributes it, with its authored span, not only to a
        // Terminal coordinate.
        assert!(
            text.contains("reached through machine `CheckingAssert::check`"),
            "{label}: the crash helper is attributed to the selected candidate that reaches it: {text}"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("declared here") && diagnostic.source_span.is_some()
            }),
            "{label}: the contributing declaration is spanned: {text}"
        );
        assert!(
            !text.contains("evidence is insufficient"),
            "{label}: a witnessed prohibited site is not an evidence gap: {text}"
        );
    }
}

#[test]
fn native_route_shares_the_exclusion_verdict() {
    for (label, rollback) in [
        ("native-optimized", OptimizationRollback::default()),
        ("native-rolled-back", rolled_back()),
    ] {
        let diagnostics = compile_one(
            "checking-app",
            RequestedCompileProduct::NativeArtifact,
            rollback.clone(),
            label,
        )
        .expect_err("native admission rejects the checking composition before realization");
        let text = messages(&diagnostics);
        assert!(
            text.contains("behavior exclusion violated: crash cause Trap"),
            "{text}"
        );

        let report = compile_one(
            "no-op-app",
            RequestedCompileProduct::NativeArtifact,
            rollback,
            label,
        )
        .unwrap_or_else(|diagnostics| {
            panic!(
                "{label}: no-op native compilation: {}",
                messages(&diagnostics)
            )
        });
        let directory = build_dir(label);
        let published = report
            .publish_retained_native_artifact(&directory)
            .expect("publish the no-op package's checked native executable");
        let executable = published
            .checked_native_executable_path()
            .expect("checked executable");
        assert!(executable.is_file());
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert!(
            std::process::Command::new(executable)
                .status()
                .expect("run no-op package")
                .success()
        );
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        eprintln!("SKIP: Linux x86-64 package executable cannot run on this host");
        std::fs::remove_dir_all(directory).expect("remove native test publication");
    }
}

#[test]
fn silent_ordinary_logger_satisfies_the_service_exclusion_despite_its_allowance() {
    let report = compile_one(
        "quiet-logger-app",
        RequestedCompileProduct::TerminalArtifact,
        OptimizationRollback::default(),
        "quiet-logger",
    )
    .unwrap_or_else(|diagnostics| {
        panic!("a silent ordinary logger passes a Sink exclusion: {diagnostics:#?}")
    });
    let retained = report
        .into_retained_terminal_artifact()
        .expect("retained Terminal product");
    let proposal = retained
        .native_realization_proposal()
        .expect("retained native proposal");
    // The retained exclusion set carries the service resolved in this
    // artifact, or nothing when the composition never declares it; either
    // way no crash cause was authored.
    assert!(proposal.behavior_exclusions().crash_causes().is_empty());
}

#[test]
fn silent_provider_for_an_actual_service_invocation_is_prohibited() {
    let diagnostics = compile_one(
        "sink-app",
        RequestedCompileProduct::TerminalArtifact,
        OptimizationRollback::default(),
        "sink-invocation",
    )
    .expect_err("an abstract Sink invocation counts even behind a silent provider");
    let text = messages(&diagnostics);
    assert!(
        text.contains("behavior exclusion violated: service `Sink`"),
        "prohibited possible behavior, named by service identity: {text}"
    );
    assert!(
        text.contains("call to boundary"),
        "the site is the boundary invocation: {text}"
    );
    assert!(
        !text.contains("evidence is insufficient"),
        "a witnessed invocation is not an evidence gap: {text}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("behavior exclusion violated")
                && diagnostic.source_span.is_some()
        }),
        "the rejection points at the authored exclude_service span: {text}"
    );
}
