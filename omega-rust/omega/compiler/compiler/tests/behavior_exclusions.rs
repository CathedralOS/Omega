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

/// The application `app` composed with `assert-kit` under the alias its
/// source imports (`use assert_kit::main;`).
fn inputs(app: &str) -> PackageCompilationInputs {
    let root = fixture_root();
    PackageCompilationInputs::new_package(
        identity(0x61),
        vec![
            PackageSourceBinding::new(identity(0x61), app, root.join(app)),
            PackageSourceBinding::new(identity(0x62), "assert-kit", root.join("assert-kit")),
        ],
        vec![PackageDependencyBinding::new(
            identity(0x61),
            "assert_kit",
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
    .with_package_inputs(inputs(app))
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
    let diagnostics = compile_one(
        "checking-app",
        RequestedCompileProduct::NativeArtifact,
        OptimizationRollback::default(),
        "checking-native",
    )
    .expect_err("native admission rejects the checking composition before realization");
    let text = messages(&diagnostics);
    assert!(
        text.contains("behavior exclusion violated: crash cause Trap"),
        "{text}"
    );

    // The no-op composition passes the exclusion; what stops its native
    // product today is the boundary requirement's own `crashes Trap`
    // contract, which terminal-psi-to-abstract-operations does not yet
    // consume. That is an explicit realization limit, not an exclusion
    // verdict, and this assertion flips when native lowering admits it.
    let diagnostics = compile_one(
        "no-op-app",
        RequestedCompileProduct::NativeArtifact,
        OptimizationRollback::default(),
        "no-op-native",
    )
    .expect_err("native lowering still refuses a boundary crash contract");
    let text = messages(&diagnostics);
    assert!(
        !text.contains("behavior exclusion"),
        "the no-op composition must not be rejected by the exclusion: {text}"
    );
    assert!(
        text.contains("UnsupportedBoundaryCrashContract"),
        "the recorded limit is the boundary crash contract: {text}"
    );
}
