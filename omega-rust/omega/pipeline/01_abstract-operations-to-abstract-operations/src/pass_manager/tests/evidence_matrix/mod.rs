//! Optimizer module role: stage group. Per-selection evidence matrices through the public phase entrance.
//!
//! Every exact `PsiOptimization` selection member gets the same matrix through
//! `run_psi_pipeline`, `publish_optimization_run`, and `optimize_abstract_operations`:
//! a positive fixture that commits, a foreign workload that declines, a boundary
//! workload at the selection's admission edge, sibling-selection and
//! empty-selection identity legs, the measured five-axis work-budget boundary,
//! run determinism, a fixed-point leg whose legal second input is the published
//! run's transformed unit, and corruption legs the independent publication
//! replay must reject. Malformed carriers are refused at both byte admission
//! boundaries. The session-level relocation step shares the same entrance
//! matrix; its retained commit names the scheduled relocation roster.

mod control_flow_cleanup;
mod copy_propagation;
mod dead_pure_scalar_elimination;
mod global_value_numbering;
mod loop_invariant_scalar_motion;
mod proof_check_elision;
mod representation_specialization;
mod sparse_conditional_constant_propagation;
mod state_specialization;

use super::super::{
    ExternalDecisionReplayError, OptimizationRunError, VerifiedPsiOptimizationSession,
};
use super::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, budget, verified_empty_unit,
};
use crate::{
    OptimizedAbstractProjectionError, optimize_abstract_operations, publish_optimization_run,
};
use crate::{replay_psi_pipeline, run_psi_pipeline};
use optimization_core::OptimizationRuleIdentity;
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationUnit;

/// One exact selection member's entrance-matrix case data.
pub(super) struct SelectionMatrix {
    /// The exact `Optimization` selection member under test.
    pub selection: Optimization,
    /// The leaf rule that must commit on the positive workload.
    pub expected_rule: fn() -> OptimizationRuleIdentity,
    /// Commit count on the positive workload.
    pub expected_commits: usize,
    /// Workload where the selection commits.
    pub positive: fn() -> VerifiedPsiOptimizationUnit,
    /// A sibling `Optimization` selection that provably declines the positive
    /// workload, isolating this member's disable policy.
    pub sibling: Optimization,
}

pub(super) fn selections_of(selection: Optimization) -> OptimizationSelections {
    OptimizationSelections::new([selection]).unwrap()
}

/// The complete public entrance: `optimize_abstract_operations` builds the
/// unit, runs, and publishes in one call, producing the same commits.
pub(super) fn assert_full_entrance_leg(case: &SelectionMatrix) {
    let unit = (case.positive)();
    let selections = selections_of(case.selection);
    let plan = optimize_abstract_operations(
        unit.input().clone(),
        &selections,
        &selections.project_psi(),
        budget(64),
    )
    .unwrap();
    assert_eq!(plan.commits().len(), case.expected_commits);
    assert!(
        plan.commits()
            .iter()
            .any(|commit| commit.rule == (case.expected_rule)())
    );
    assert_eq!(plan.selections(), &selections);
    assert_eq!(plan.psi_selections(), &selections);
}

/// Negative: the empty workload declines with the selection enabled.
pub(super) fn assert_negative_leg(case: &SelectionMatrix) {
    let unit = verified_empty_unit();
    let input_identity = unit.unit().identity;
    let selections = selections_of(case.selection);
    let run = run_psi_pipeline(unit, &selections, budget(64)).unwrap();
    assert!(run.commits().is_empty());
    assert_eq!(run.psi_selections(), &selections);
    assert_eq!(run.session().unit().identity, input_identity);
    let plan = publish_optimization_run(run).unwrap();
    assert_eq!(plan.unit().identity, input_identity);
    assert!(plan.transformation_ledger().records().is_empty());
}

/// Disabled policy: under a sibling selection the positive workload is
/// untouched and this member contributes no commit.
pub(super) fn assert_disabled_leg(case: &SelectionMatrix) {
    let unit = (case.positive)();
    let input_identity = unit.unit().identity;
    let sibling = selections_of(case.sibling);
    let run = run_psi_pipeline(unit, &sibling, budget(64)).unwrap();
    assert_eq!(run.psi_selections(), &sibling);
    assert!(run.commits().is_empty());
    assert_eq!(run.session().unit().identity, input_identity);
    let plan = publish_optimization_run(run).unwrap();
    assert_eq!(plan.unit().identity, input_identity);
}

/// Empty selection: with no member selected the phase runs no pass, and the
/// positive workload publishes byte-exact — the rollback receipt's empty
/// effective set shares this identity route.
pub(super) fn assert_empty_selection_leg(case: &SelectionMatrix) {
    let unit = (case.positive)();
    let input_identity = unit.unit().identity;
    let selections = OptimizationSelections::default();
    let run = run_psi_pipeline(unit, &selections, budget(64)).unwrap();
    assert_eq!(run.psi_selections(), &selections);
    assert!(run.commits().is_empty());
    assert_eq!(run.session().unit().identity, input_identity);
    let plan = publish_optimization_run(run).unwrap();
    assert_eq!(plan.unit().identity, input_identity);
    assert!(plan.transformation_ledger().records().is_empty());
}

/// Measured budget: the run's recorded usage is admitted at the exact count on
/// every axis and refused one step below wherever a legal smaller budget
/// exists.
pub(super) fn assert_measured_budget_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let measured = run_psi_pipeline((case.positive)(), &selections, budget(64))
        .unwrap()
        .usage;
    let generous = OptimizationWorkBudget::new(96, 64, 64, 64, 64).unwrap();
    let axes = [
        ("rule evaluations", measured.rule_evaluations),
        ("candidates", measured.candidates),
        ("validation steps", measured.validation_steps),
        ("commits", measured.commits),
        ("iterations", measured.iterations),
    ];
    for (index, (name, amount)) in axes.iter().copied().enumerate() {
        assert!(amount >= 1, "a positive run uses every budget axis");
        let build = |value: u64| {
            let mut parts = [
                generous.rule_evaluations(),
                generous.candidates(),
                generous.validation_steps(),
                generous.commits(),
                generous.iterations(),
            ];
            parts[index] = value;
            OptimizationWorkBudget::new(parts[0], parts[1], parts[2], parts[3], parts[4]).unwrap()
        };
        run_psi_pipeline((case.positive)(), &selections, build(amount))
            .expect("the measured usage must admit at the exact count");
        if amount > 1 {
            let error =
                run_psi_pipeline((case.positive)(), &selections, build(amount - 1)).unwrap_err();
            assert_eq!(error, OptimizationRunError::WorkBudgetExhausted(name));
        }
    }
}

/// Corruption: forgeries on every published run axis are rejected by the
/// independent publication replay.
pub(super) fn assert_corruption_legs(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let foreign = run_psi_pipeline(
        verified_empty_unit(),
        &OptimizationSelections::default(),
        budget(64),
    )
    .unwrap();

    // A drifted phase projection is refused before any replay work.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.psi_selections = selections_of(case.sibling);
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::PsiSelectionProjectionMismatch)
    ));

    // A foreign complete-selection projection is refused at the public phase
    // entrance before any pass runs.
    assert!(matches!(
        optimize_abstract_operations(
            (case.positive)().input().clone(),
            &selections,
            &selections_of(case.sibling).project_psi(),
            budget(64),
        ),
        Err(crate::AbstractOptimizationError::Run(
            OptimizationRunError::SelectionRegistryMismatch
        ))
    ));

    // A forged commit output identity fails the independent commit replay.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let commit = run.commits.first_mut().expect("positive run has a commit");
    commit.output = commit.input;
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::CommitReplayMismatch)
    ));

    // A foreign session swaps the replay input: the first commit no longer
    // lands on the rebuilt unit.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.session = foreign.session;
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::CommitReplayMismatch)
            | Err(OptimizedAbstractProjectionError::FinalUnitReplayMismatch)
            | Err(OptimizedAbstractProjectionError::InitialUnitProjection)
    ));

    // A foreign ledger fails the ledger-to-commit join.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.transformation_ledger = foreign.transformation_ledger.clone();
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::LedgerCommitMismatch)
    ));

    // An emptied manifest roster fails the rule-set and usage replay.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.pass_manifests = foreign.pass_manifests.clone();
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::ManifestUsageMismatch)
            | Err(OptimizedAbstractProjectionError::AppliedDecisionCustody { .. })
    ));

    // A foreign baseline log fails the validated-candidate join.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.decisions = foreign.decisions.clone();
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::AppliedDecisionCustody { .. })
            | Err(OptimizedAbstractProjectionError::ManifestUsageMismatch)
    ));

    // A foreign external-decision log fails the recorded-policy mirror.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.external_decisions = foreign.external_decisions.clone();
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::ExternalDecisionRecordingMismatch)
    ));

    // An inflated usage record disagrees with the manifest accounting.
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    run.usage.commits += 5;
    assert!(matches!(
        publish_optimization_run(run),
        Err(OptimizedAbstractProjectionError::ManifestUsageMismatch)
    ));
}

/// Malformed carriers: strict external-decision schema decoding and the
/// artifact admission boundary both fail closed.
pub(super) fn assert_malformed_carrier_legs(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    assert!(matches!(
        replay_psi_pipeline(
            (case.positive)(),
            &selections,
            budget(64),
            &[0xde, 0xad, 0xbe, 0xef]
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::Schema(_)
        ))
    ));
    assert!(
        terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &[0xde, 0xad, 0xbe, 0xef],
                proof_bytes: &[],
                obligation_ledger_bytes: None,
            },
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
    // A truncated artifact cannot decode either.
    assert!(
        terminal_psi_to_abstract_operations::lower_artifact(
            terminal_psi_to_abstract_operations::ArtifactSections {
                semantic_bytes: &[0x00],
                proof_bytes: &[0x00],
                obligation_ledger_bytes: None,
            },
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}
