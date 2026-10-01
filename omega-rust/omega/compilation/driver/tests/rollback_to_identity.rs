//! Rolling back the only selected rule has to rejoin the ordinary
//! no-selection route, and a test has to actually supply the overlay to say so.
//!
//! The corpus fixture `optimizer/rollback_to_no_selection_empty_entry` enables
//! `ControlFlowCleanup` and its comment claims the release overlay disables it.
//! Nothing in the corpus route supplies that overlay —
//! `corpus_runner::build_native_fixture` builds its request without one — so
//! running the fixture exercises the ordinary selected path and reports green
//! while the claim goes unchecked.
//!
//! These tests supply the overlay through the compiler's own request, read the
//! receipt it settles, and compare the published executable against a control
//! build that selects nothing. That control is what makes the rejoin claim
//! falsifiable: without it, "effective is empty" is a statement about the
//! receipt rather than about what was published.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use optimization_core::{Optimization, OptimizationRollback};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

const TARGET: &str = "macos_arm64";

fn build_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

struct Produced {
    /// The canonical semantic module. A Psi-stage rewrite shows here; the
    /// native executable normalizes `ControlFlowCleanup` away, so comparing
    /// executables cannot see whether the rule ran.
    semantic: Vec<u8>,
    receipt: Option<String>,
}

/// Compile one fixture to its retained Terminal product under `rollback`.
fn produce(fixture: &str, rollback: OptimizationRollback, name: &str) -> Produced {
    let main = repo_root().join(fixture);
    let package_inputs = reviewed_repository_fixture_package_inputs(&main, Some(TARGET))
        .unwrap_or_else(|diagnostics| panic!("{fixture} package inputs: {diagnostics:#?}"));
    let root = build_dir(name);
    let _ = std::fs::remove_dir_all(&root);
    let mut request = CompileRequest::new(CompileOptions {
        root_path: main,
        build_dir: Some(root.clone()),
        target_name: Some(TARGET.to_owned()),
    })
    .with_requested_product(RequestedCompileProduct::TerminalArtifact)
    .with_optimization_rollback(rollback);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{fixture} must reach Terminal: {diagnostics:#?}"));
    let receipt = report
        .optimization_rollback_receipt()
        .map(ToString::to_string);
    let retained = report
        .into_retained_terminal_artifact()
        .unwrap_or_else(|| panic!("{fixture} retains a Terminal artifact"));
    let semantic = retained.artifact().semantic_bytes().to_vec();
    let _ = std::fs::remove_dir_all(&root);
    Produced { semantic, receipt }
}

const SELECTED: &str =
    "omega-rust/omega/compilation/driver/tests/fixtures/rollback_cfc_selected/main.omg";
const CONTROL: &str =
    "omega-rust/omega/compilation/driver/tests/fixtures/rollback_cfc_control/main.omg";

/// The overlay disables the sole authored rule, so the receipt names it as both
/// requested and applied and leaves the effective set empty.
#[test]
fn disabling_the_only_selected_rule_settles_an_exact_receipt() {
    let rolled_back = produce(
        SELECTED,
        OptimizationRollback::new([Optimization::ControlFlowCleanup]).expect("one disabled rule"),
        "rollback-receipt",
    );
    assert_eq!(
        rolled_back.receipt.as_deref(),
        Some("requested=[ControlFlowCleanup]; applied=[ControlFlowCleanup]; effective=[]"),
        "the settled receipt names requested, applied and effective exactly"
    );
}

/// The rejoin claim itself: with its only rule rolled back, the fixture
/// publishes exactly what a build that selects nothing publishes. Comparing
/// bytes is what makes this falsifiable — an empty effective set in the receipt
/// says nothing about the artifact.
#[test]
fn a_rolled_back_selection_publishes_what_no_selection_publishes() {
    let rolled_back = produce(
        SELECTED,
        OptimizationRollback::new([Optimization::ControlFlowCleanup]).expect("one disabled rule"),
        "rollback-rejoin",
    );
    let control = produce(CONTROL, OptimizationRollback::default(), "rollback-control");
    assert!(
        control.receipt.is_none(),
        "an empty request creates no rollback receipt: {:?}",
        control.receipt
    );
    assert_eq!(
        rolled_back.semantic.len(),
        control.semantic.len(),
        "the rolled-back build publishes a module of the control's size"
    );
    assert!(
        rolled_back.semantic == control.semantic,
        "the rolled-back build publishes the control's exact module"
    );
}

/// The negative side of the same measurement: without an overlay the authored
/// selection stands and no receipt is settled. This is the state the corpus
/// route actually exercises, which is why it could not have checked the claim.
#[test]
fn the_same_fixture_without_an_overlay_settles_no_receipt() {
    let ordinary = produce(SELECTED, OptimizationRollback::default(), "rollback-absent");
    assert!(
        ordinary.receipt.is_none(),
        "no overlay means no rollback provenance: {:?}",
        ordinary.receipt
    );
}

/// Guards the comparison above against being vacuous. If the authored
/// selection published the control's module even without an overlay, then
/// module equality under rollback would be a restatement of the receipt
/// rather than evidence about what was published. This is the assertion that
/// `ControlFlowCleanup` actually fires on this source.
#[test]
fn the_selected_rule_changes_the_module_it_is_rolled_back_from() {
    let selected_no_overlay = produce(
        SELECTED,
        OptimizationRollback::default(),
        "rollback-probe-a",
    );
    let control = produce(CONTROL, OptimizationRollback::default(), "rollback-probe-b");
    assert!(
        selected_no_overlay.semantic != control.semantic,
        "ControlFlowCleanup must change this module, or rolling it back proves \
         nothing: the fold needs a constant-condition transition whose untaken \
         arm leaves a block unreachable"
    );
}
