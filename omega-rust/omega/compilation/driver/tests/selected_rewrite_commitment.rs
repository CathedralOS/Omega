//! The corpus `optimizer/*_selection_compile` and `*_selection_exit`
//! fixtures select one exact machine-stage rewrite family each, and their
//! `expected_native_exit.txt` oracles now execute the result. Executing
//! alone cannot show which rule committed: a compile that silently skipped
//! the fold would still emit a runnable program — it would just exit
//! differently. This suite pins the other half of the claim — that the
//! named rule's transformation is what the published artifact carries.
//!
//! For each fixture the compiler's own product route is driven twice:
//! once as authored, and once with the named rule rolled back through the
//! release overlay. The retained artifact is the published executable's own
//! join — the same `NativeArtifact` `publish_compilation` installs — so it
//! replays in full: `validate` re-derives every physical-evidence child and
//! recomputes the identity, and `from_replayed_parts` rebuilds the whole
//! artifact from its retained parts. The two builds differ only in the
//! overlay, so when the rolled-back artifact's instruction bytes differ, the
//! difference is exactly the named rule's commitment — no source change can
//! explain it. `rollback_to_identity.rs` covers the receipt alone; the byte
//! differential is what makes the commitment falsifiable.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct, compile};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use optimization_core::{Optimization, OptimizationRollback};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE_ID: AtomicU64 = AtomicU64::new(1);

const TARGET: &str = "linux_x86_64";

fn build_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omega-{name}-{}-{}",
        std::process::id(),
        NEXT_FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

struct Produced {
    /// The complete retained native join — object, image, provider
    /// realization and physical evidence — before publication installs it.
    artifact: native_artifact::NativeArtifact,
    receipt: Option<String>,
}

/// Compile one fixture to its retained native artifact under `rollback`.
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
    .with_requested_product(RequestedCompileProduct::NativeArtifact)
    .with_optimization_rollback(rollback);
    if let Some(package_inputs) = package_inputs {
        request = request.with_package_inputs(package_inputs);
    }
    let report = compile(request)
        .and_then(compiler::CompileOutcomes::into_single_report)
        .unwrap_or_else(|diagnostics| panic!("{fixture} must reach native: {diagnostics:#?}"));
    let receipt = report
        .optimization_rollback_receipt()
        .map(ToString::to_string);
    let artifact = report
        .into_retained_native_artifact()
        .unwrap_or_else(|| panic!("{fixture} retains a native artifact"));
    let _ = std::fs::remove_dir_all(&root);
    Produced { artifact, receipt }
}

/// Replay one retained artifact in place and once more from its own parts.
/// `validate` re-derives the physical evidence the published join asserts;
/// `from_replayed_parts` rebuilds the artifact from its retained parts,
/// recomputes the identity through the same derivation and validates again,
/// so the returned artifact exists only when the whole join replayed.
fn replay(
    artifact: native_artifact::NativeArtifact,
    fixture: &str,
) -> native_artifact::NativeArtifact {
    artifact
        .validate()
        .unwrap_or_else(|error| panic!("{fixture} retained artifact replay: {error}"));
    native_artifact::NativeArtifact::from_replayed_parts(artifact.into_parts())
        .unwrap_or_else(|error| panic!("{fixture} artifact parts replay: {error}"))
}

struct CommitmentCase {
    fixture: &'static str,
    /// The one authored rule this witness rolls back by exact name.
    rule: Optimization,
    /// The exact settled receipt text once `rule` is subtracted.
    rolled_back_receipt: &'static str,
}

/// Drive the authored selection and its exact-name rollback, then compare
/// the two published instruction byte streams. The fold removing or keeping
/// a conditional branch, a dead or duplicated compare, is a byte-visible
/// difference in the object the retained artifact carries.
fn committed_rule_changes_the_published_bytes(case: &CommitmentCase) {
    let selected = produce(
        case.fixture,
        OptimizationRollback::default(),
        "commitment-selected",
    );
    assert!(
        selected.receipt.is_none(),
        "{}: no overlay means no rollback provenance: {:?}",
        case.fixture,
        selected.receipt
    );
    let rolled_back = produce(
        case.fixture,
        OptimizationRollback::new([case.rule]).expect("one disabled rule"),
        "commitment-rolled-back",
    );
    assert_eq!(
        rolled_back.receipt.as_deref(),
        Some(case.rolled_back_receipt),
        "{}: the settled receipt names requested, applied and effective exactly",
        case.fixture
    );

    let selected_artifact = replay(selected.artifact, case.fixture);
    let rolled_artifact = replay(rolled_back.artifact, case.fixture);
    assert_ne!(
        selected_artifact.object().text_bytes(),
        rolled_artifact.object().text_bytes(),
        "{}: rolling back {} must change the published instruction bytes — \
         equality would mean the rule never reached the artifact",
        case.fixture,
        case.rule.build_case_name()
    );
    assert_ne!(
        selected_artifact.identity(),
        rolled_artifact.identity(),
        "{}: the published identity differs under the exact rollback",
        case.fixture
    );
}

/// `constant_branch_selection_compile` selects `SelectedConstantBranchFoldV1`
/// alone; folding the `7 < 7` conditional decides the branch and the program
/// exits 0 only when `decide(7i64)` still returns 43.
#[test]
fn constant_branch_fold_commits_to_the_published_artifact() {
    committed_rule_changes_the_published_bytes(&CommitmentCase {
        fixture: "tests/omega/pass/optimizer/constant_branch_selection_compile/main.omg",
        rule: Optimization::SelectedConstantBranchFoldV1,
        rolled_back_receipt: "requested=[SelectedConstantBranchFoldV1]; \
                              applied=[SelectedConstantBranchFoldV1]; effective=[]",
    });
}

/// `dead_compare_selection_compile` selects the two constant-flag folds and
/// `SelectedDeadCompareRemovalV1`: folding the branch leaves the compare's
/// flag publication unread, and the removal deletes it. Rolling back only
/// the removal keeps both folds, so the emitted `cmp` stays and the program
/// still exits 0 — while the object bytes name the rule that left it.
#[test]
fn dead_compare_removal_commits_to_the_published_artifact() {
    committed_rule_changes_the_published_bytes(&CommitmentCase {
        fixture: "tests/omega/pass/optimizer/dead_compare_selection_compile/main.omg",
        rule: Optimization::SelectedDeadCompareRemovalV1,
        rolled_back_receipt: "requested=[SelectedDeadCompareRemovalV1]; \
                              applied=[SelectedDeadCompareRemovalV1]; \
                              effective=[SelectedConstantBooleanFoldV1, \
                              SelectedConstantBranchFoldV1]",
    });
}

/// `equivalent_compare_selection_compile` selects
/// `SelectedEquivalentCompareRemovalV1` alone: the second `x < 5` compares
/// operands already proven equal to the first's, so its flag publication is
/// removed and `probe(7i64)` still returns 0.
#[test]
fn equivalent_compare_removal_commits_to_the_published_artifact() {
    committed_rule_changes_the_published_bytes(&CommitmentCase {
        fixture: "tests/omega/pass/optimizer/equivalent_compare_selection_compile/main.omg",
        rule: Optimization::SelectedEquivalentCompareRemovalV1,
        rolled_back_receipt: "requested=[SelectedEquivalentCompareRemovalV1]; \
                              applied=[SelectedEquivalentCompareRemovalV1]; effective=[]",
    });
}

/// `redundant_compare_selection_compile` selects
/// `SelectedRedundantCompareRemovalV1` alone: `left < right` twice against
/// the same operands republishes identical flags, so the removal deletes the
/// second compare and `probe(7i64, 9i64)` still returns 1.
#[test]
fn redundant_compare_removal_commits_to_the_published_artifact() {
    committed_rule_changes_the_published_bytes(&CommitmentCase {
        fixture: "tests/omega/pass/optimizer/redundant_compare_selection_compile/main.omg",
        rule: Optimization::SelectedRedundantCompareRemovalV1,
        rolled_back_receipt: "requested=[SelectedRedundantCompareRemovalV1]; \
                              applied=[SelectedRedundantCompareRemovalV1]; effective=[]",
    });
}

/// `copied_call_operand_selection_exit` selects
/// `SelectedCopiedCallOperandFoldV1` alone: `scale(14i64, 3i64)` stages both
/// scalar arguments through `CopyI64` records, and the fold rebinds the
/// call's `Use` operand registers to the copies' sources. The rebound
/// operand encoding is the byte difference the rollback must restore, and
/// the program still exits 70 only when `scale` returned 42 either way.
#[test]
fn copied_call_operand_fold_commits_to_the_published_artifact() {
    committed_rule_changes_the_published_bytes(&CommitmentCase {
        fixture: "tests/omega/pass/optimizer/copied_call_operand_selection_exit/main.omg",
        rule: Optimization::SelectedCopiedCallOperandFoldV1,
        rolled_back_receipt: "requested=[SelectedCopiedCallOperandFoldV1]; \
                              applied=[SelectedCopiedCallOperandFoldV1]; effective=[]",
    });
}

/// `stored_load_forwarding_selection_exit` selects
/// `SelectedStoredLoadForwardingV1` alone: the store of 43 into the context
/// field and the immediate reload forward the stored register, replacing the
/// field reload with a register copy. Rolling the rule back retains the
/// reload, so the emitted text differs — the fixture's own README records
/// the initial witness's 76- versus 80-byte extents — and `main` still
/// traps only when the observed field read back wrong.
#[test]
fn stored_load_forwarding_commits_to_the_published_artifact() {
    committed_rule_changes_the_published_bytes(&CommitmentCase {
        fixture: "tests/omega/pass/optimizer/stored_load_forwarding_selection_exit/main.omg",
        rule: Optimization::SelectedStoredLoadForwardingV1,
        rolled_back_receipt: "requested=[SelectedStoredLoadForwardingV1]; \
                              applied=[SelectedStoredLoadForwardingV1]; effective=[]",
    });
}
