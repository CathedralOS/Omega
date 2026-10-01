//! Two selected rules have to compose: neither one's effect may be lost
//! because the other was also selected.
//!
//! `PER-RULE-COVERAGE` asks each rule for a composition axis and no rule had
//! one — a screen of the stage-06 rule tests finds rollback, composition and
//! budget absent for all six. Selecting rules one at a time proves each works
//! alone, and a green corpus observation proves only that a module was
//! published, not that both rules still reached it.
//!
//! The four builds here differ only in the compiler's own rollback overlay
//! over one authored selection, so no source difference can explain a
//! difference in the module: `{}`, `{CopyPropagation}`,
//! `{DeadPureScalarElimination}` and both. Comparing the semantic module is
//! what makes this observable at all — native emission normalizes these
//! Psi-stage rewrites away.
//!
//! On this fixture the modules measure 1079, 1051, 985 and 957 bytes, so each
//! rule's saving is exactly its own — 28 and 94 — and both together save
//! their sum. The assertions deliberately check distinctness rather than
//! those sizes: a rule may legitimately change how many bytes it saves, while
//! losing an effect to its neighbour is always wrong.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use optimization_core::{Optimization, OptimizationRollback};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

const TARGET: &str = "macos_arm64";
const FIXTURE: &str =
    "omega-rust/omega/compilation/driver/tests/fixtures/composed_rewrites/main.omg";

fn build_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

/// The semantic module the fixture publishes with `disabled` rolled back out
/// of its authored selection.
fn published(disabled: &[Optimization], name: &str) -> Vec<u8> {
    let main = repo_root().join(FIXTURE);
    let package_inputs = reviewed_repository_fixture_package_inputs(&main, Some(TARGET))
        .unwrap_or_else(|diagnostics| panic!("{name} package inputs: {diagnostics:#?}"));
    let root = build_dir(name);
    let _ = std::fs::remove_dir_all(&root);
    let mut request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(root.clone()),
        target_name: Some(TARGET.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact);
    if !disabled.is_empty() {
        request = request.with_optimization_rollback(
            OptimizationRollback::new(disabled.iter().copied()).expect("a nonempty rollback"),
        );
    }
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{name} must reach Terminal: {diagnostics:#?}"));
    let published = report
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{name} retains a Terminal artifact"))
        .artifact()
        .semantic_bytes()
        .to_vec();
    let _ = std::fs::remove_dir_all(&root);
    published
}

/// Both rules selected together must publish a module that differs from each
/// rule selected alone. Equality with one of them would mean the other rule
/// stopped reaching the module once its neighbour was selected, which is the
/// composition failure this axis exists to catch.
#[test]
fn composing_two_rules_keeps_both_effects() {
    let neither = published(
        &[
            Optimization::CopyPropagation,
            Optimization::DeadPureScalarElimination,
        ],
        "composed-neither",
    );
    let copy_only = published(
        &[Optimization::DeadPureScalarElimination],
        "composed-copy-only",
    );
    let dead_only = published(&[Optimization::CopyPropagation], "composed-dead-only");
    let both = published(&[], "composed-both");

    // The guard against a vacuous comparison: a rule that changes nothing on
    // this source makes every claim below a restatement of the baseline.
    assert_ne!(
        copy_only, neither,
        "copy propagation must change this module, or composing it proves nothing"
    );
    assert_ne!(
        dead_only, neither,
        "dead pure scalar elimination must change this module, or composing it \
         proves nothing"
    );

    assert_ne!(
        both, copy_only,
        "selecting both lost dead pure scalar elimination's effect"
    );
    assert_ne!(
        both, dead_only,
        "selecting both lost copy propagation's effect"
    );
    assert_ne!(both, neither, "selecting both changed nothing at all");
}
