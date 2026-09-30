//! A rewrite a pass proposed and then withheld has to reach the person
//! compiling, or the decision is indistinguishable from the pass finding
//! nothing to do.
//!
//! Stage 06 already records the decision on its own result. This witnesses the
//! rest of the route: production reads the decisions off the stage result
//! before publication consumes it, projects them onto the shared optimization
//! reason vocabulary, and the compile report carries them out for the CLI to
//! name. The published artifact is identical either way, so nothing here
//! asserts a difference in bytes — only that the explanation survived.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use optimization::PsiOptimization;
use optimization_core::OptimizationReasonCode;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

fn build_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Compile one repository fixture to its retained Terminal product and return
/// the decisions its report carries.
fn withheld_rewrites_of(
    fixture: &str,
    target: &str,
) -> Vec<(PsiOptimization, OptimizationReasonCode)> {
    let main = repo_root().join(fixture);
    let package_inputs = reviewed_repository_fixture_package_inputs(&main, Some(target))
        .unwrap_or_else(|diagnostics| panic!("{fixture} package inputs: {diagnostics:#?}"));
    let root = build_dir("withheld-rewrites");
    let _ = std::fs::remove_dir_all(&root);
    let mut request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(root.clone()),
        target_name: Some(target.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{fixture} must reach Terminal: {diagnostics:#?}"));
    let withheld = report
        .withheld_rewrites()
        .iter()
        .map(|withheld| (withheld.optimization(), withheld.reason()))
        .collect();
    let _ = std::fs::remove_dir_all(&root);
    withheld
}

/// The proof-bearing customer. Its runtime index site gives the machine a
/// reconstructed obligation, so the two-arm literal join retains an interval
/// invariant naming the values both selected passes would rewrite. Each pass
/// proposes a complete rewrite and its own validator reconstructs a different
/// proof question from the proposal, so each withholds — and the report names
/// the exact rule and the reason.
#[test]
fn a_consumed_join_invariant_reports_both_withheld_rewrites() {
    let withheld = withheld_rewrites_of(
        "omega-rust/omega/compilation/driver/tests/fixtures/withheld_rewrite_report/main.omg",
        "macos_arm64",
    );
    assert!(
        withheld.contains(&(
            PsiOptimization::CopyPropagation,
            OptimizationReasonCode::ValidationFailed
        )),
        "copy propagation's withheld rewrite must reach the report: {withheld:?}"
    );
    assert!(
        withheld.contains(&(
            PsiOptimization::DeadPureScalarElimination,
            OptimizationReasonCode::ValidationFailed
        )),
        "dead scalar elimination's withheld rewrite must reach the report: {withheld:?}"
    );
}

/// The control. This fixture selects the same passes with no consumed join
/// invariant to refuse at, so the passes either rewrite or find nothing and
/// the report carries no decision. Without it, a route that reported every
/// selected pass would pass the test above for the wrong reason.
#[test]
fn a_selection_without_a_consumed_invariant_reports_no_withheld_rewrite() {
    let withheld = withheld_rewrites_of(
        "tests/omega/pass/terminal_psi/selected_optimizer_component/main.omg",
        "linux_x86_64",
    );
    assert!(
        withheld.is_empty(),
        "no pass withheld a rewrite here: {withheld:?}"
    );
}
