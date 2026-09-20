//! Optimizer module role: stage group. Per-selection evidence matrices through the public phase entrance.
//!
//! Every exact `PsiOptimization` selection member gets the same matrix through
//! `run_psi_pipeline`, `publish_optimization_run`, and `optimize_abstract_operations`:
//! a positive fixture that commits, a foreign workload that declines, a boundary
//! workload at the selection's admission edge, a sibling-selection identity leg,
//! the measured five-axis work-budget boundary, run determinism, a fixed-point
//! leg whose legal second input is the published run's transformed unit, and
//! corruption legs the independent publication replay must reject. Malformed
//! carriers are refused at both byte admission boundaries.

mod control_flow_cleanup;
mod copy_propagation;
mod dead_pure_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;
mod state_specialization;

use super::super::{
    ExternalDecisionReplayError, OptimizationRunError, VerifiedPsiOptimizationSession,
    replay_psi_pipeline, run_psi_pipeline, run_unit,
};
use super::{
    Optimization, OptimizationSelections, OptimizationWorkBudget, budget, verified_empty_unit,
};
use crate::{
    OptimizedAbstractProjectionError, built_in_psi_registry, optimize_abstract_operations,
    publish_optimization_run,
};
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
    /// Near-miss workload: real content at the selection's admission edge that
    /// must decline.
    pub boundary: fn() -> VerifiedPsiOptimizationUnit,
    /// A sibling `Optimization` selection that provably declines the positive
    /// workload, isolating this member's disable policy.
    pub sibling: Optimization,
}

pub(super) fn selections_of(selection: Optimization) -> OptimizationSelections {
    OptimizationSelections::new([selection]).unwrap()
}

/// Positive: the selection commits through the public entrance, and the run
/// publishes a validated plan whose unit moved.
pub(super) fn assert_positive_leg(case: &SelectionMatrix) {
    let unit = (case.positive)();
    let input_identity = unit.unit().identity;
    let selections = selections_of(case.selection);
    let run = run_psi_pipeline(unit, &selections, budget(64)).unwrap();
    assert_eq!(run.commits().len(), case.expected_commits);
    assert!(
        run.commits()
            .iter()
            .any(|commit| commit.rule == (case.expected_rule)()),
        "the positive workload must commit through the selection's rule"
    );
    assert_eq!(run.pass_manifests().len(), 1);
    assert!(run.usage().commits >= 1);
    let plan = publish_optimization_run(run).unwrap();
    assert_ne!(plan.unit().identity, input_identity);
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

/// Boundary: the near-miss workload is evaluated by the whole selected
/// registry and commits nothing.
pub(super) fn assert_boundary_leg(case: &SelectionMatrix) {
    let unit = (case.boundary)();
    let input_identity = unit.unit().identity;
    let run = run_psi_pipeline(unit, &selections_of(case.selection), budget(64)).unwrap();
    assert!(run.commits().is_empty());
    assert_eq!(run.session().unit().identity, input_identity);
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

/// Determinism: two independent pipeline runs and their publications agree on
/// every recorded evidence surface.
pub(super) fn assert_determinism_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let first = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let second = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    assert_eq!(first.session().unit(), second.session().unit());
    assert_eq!(first.commits(), second.commits());
    assert_eq!(first.usage(), second.usage());
    assert_eq!(first.decisions(), second.decisions());
    assert_eq!(first.external_decisions(), second.external_decisions());
    assert_eq!(first.pass_manifests(), second.pass_manifests());
    assert_eq!(
        first.transformation_ledger(),
        second.transformation_ledger()
    );
    assert_eq!(first.identity_bundle(), second.identity_bundle());
    let first_plan = publish_optimization_run(first).unwrap();
    let second_plan = publish_optimization_run(second).unwrap();
    assert_eq!(first_plan.plan(), second_plan.plan());
    assert_eq!(first_plan.unit().identity, second_plan.unit().identity);
    assert_eq!(first_plan.identity_bundle(), second_plan.identity_bundle());
    assert_eq!(
        first_plan.pre_physical_manifest(),
        second_plan.pre_physical_manifest()
    );
}

/// Fixed point: the published run's transformed unit is the phase's legal
/// second input. It re-admits through custody-preserving session validation
/// and re-runs the selected registry to a terminal decline.
pub(super) fn assert_fixed_point_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let registry = built_in_psi_registry(&selections).unwrap();
    let plan = publish_optimization_run(run).unwrap();
    let session = VerifiedPsiOptimizationSession::from_transformed(
        plan.verified_input().clone(),
        plan.unit().clone(),
    )
    .expect("the published unit must re-admit against the retained verified input");
    let transformed = session.unit().clone();
    let (second, commits, usage, _decisions, _manifest, ledger) =
        run_unit(transformed.clone(), &registry, budget(64)).unwrap();
    assert_eq!(second, transformed);
    assert!(commits.is_empty());
    assert_eq!(usage.commits, 0);
    assert!(ledger.records().is_empty());
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
        terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
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
        terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
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
