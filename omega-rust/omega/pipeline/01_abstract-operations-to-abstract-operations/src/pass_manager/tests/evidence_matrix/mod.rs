//! Optimizer module role: stage group. Per-selection evidence matrices through the public phase entrance.
//!
//! Every exact `PsiOptimization` selection member gets the same matrix through
//! `run_psi_pipeline`, `publish_optimization_run`, and `optimize_abstract_operations`:
//! a positive fixture that commits, a foreign workload that declines, a boundary
//! workload at the selection's admission edge, sibling-selection and
//! empty-selection identity legs, the measured five-axis work-budget boundary,
//! run determinism, a fixed-point leg whose legal second input is the published
//! run's transformed unit, downstream replay of the recorded external decision
//! log, stale and foreign-context logs refused before replay, composition
//! boundaries, and corruption legs the independent publication replay must
//! reject. Malformed carriers are refused at both byte admission boundaries.
//! The session-level relocation step shares the same entrance matrix; its
//! retained commit names the scheduled relocation roster.
//!
//! Two axes land differently at this target-neutral entrance and carry their
//! dispositions here. There is no wrong-target surface on the run path: the
//! phase owns no target operand, so the only executable wrong-target evidence
//! is the recorded policy mirror's bound target-neutral decision target —
//! `assert_stale_evidence_leg` forges a foreign target identity into the
//! replayed log's context and the replay refuses it. Unsupported composition
//! has three honest refusal surfaces rather than one: a complete selection of
//! foreign-phase members projects to an empty Psi phase and identity-publishes,
//! a mixed suite executes only its Psi members, and duplicate members and
//! multi-member single-registry requests refuse outright.

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
    ExternalDecisionContextAxis, ExternalDecisionReplayError, OptimizationRunError,
    VerifiedPsiOptimizationSession, run_registries,
};
use super::{
    ExternalDecisionContext, Optimization, OptimizationSelections, OptimizationWorkBudget, budget,
    external_log_with, verified_empty_unit,
};
use crate::rules::{built_in_psi_registry, built_in_psi_steps_for_selections};
use crate::{
    OptimizedAbstractProjectionError, RuleRegistryError, optimize_abstract_operations,
    publish_optimization_run,
};
use crate::{replay_psi_pipeline, run_psi_pipeline};
use optimization_core::{DuplicateOptimization, OptimizationRuleIdentity};
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
    /// A workload at the selection's admission edge: close enough to nominate
    /// that its clean decline means the edge, not the fixture, was tested.
    pub boundary: fn() -> VerifiedPsiOptimizationUnit,
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

/// Boundary: a workload at the selection's admission edge declines under the
/// enabled selection and publishes byte-exact.
pub(super) fn assert_boundary_leg(case: &SelectionMatrix) {
    let unit = (case.boundary)();
    let input_identity = unit.unit().identity;
    let selections = selections_of(case.selection);
    let run = run_psi_pipeline(unit, &selections, budget(64)).unwrap();
    assert_eq!(run.psi_selections(), &selections);
    assert!(
        run.commits().is_empty(),
        "the admission-edge workload must decline, not commit"
    );
    assert_eq!(run.session().unit().identity, input_identity);
    let plan = publish_optimization_run(run).unwrap();
    assert_eq!(plan.unit().identity, input_identity);
    assert!(plan.transformation_ledger().records().is_empty());
}

/// Determinism: two independent runs of the positive workload through the
/// public entrance produce identical commits, usage, decision and external
/// logs, manifests, ledger, identity bundle, and resulting unit.
pub(super) fn assert_determinism_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let first = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let second = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
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
    assert_eq!(
        first.session().unit().identity,
        second.session().unit().identity
    );
}

/// Fixed point: the published run's transformed unit is a legal second input.
/// Re-admitted through transformed-revision custody and re-run under the same
/// scheduled steps, it commits nothing and keeps the published unit's
/// identity.
///
/// Publication replay is input-rooted — it rebuilds the initial unit from the
/// run's verified input and applies the recorded commits — so a run whose base
/// is a transformed revision is not a publishable run shape, and this leg
/// asserts the re-run's empty commit list rather than a second publication.
pub(super) fn assert_fixed_point_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let projection = selections.project_psi();
    let steps = built_in_psi_steps_for_selections(projection.selections()).unwrap();
    let first = publish_optimization_run(
        run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap(),
    )
    .unwrap();
    let session = VerifiedPsiOptimizationSession::from_transformed(
        first.verified_input().clone(),
        first.unit().clone(),
    )
    .expect("the published unit re-admits as a transformed revision");
    let second = run_registries(
        session,
        &selections,
        projection.selections(),
        &steps,
        budget(64),
    )
    .unwrap();
    assert!(
        second.usage().iterations >= 1,
        "the fixed-point leg re-runs the scheduled steps, not a no-op"
    );
    assert!(
        second.commits().is_empty(),
        "the published unit is already at the selection's fixed point"
    );
    assert_eq!(
        second.session().unit().identity,
        first.unit().identity,
        "re-running the published artifact keeps the unit byte-exact"
    );
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

    // Cross-rule: a commit reattributed to a rule outside this run's recorded
    // declarations fails the commit-to-declaration join before the rule's
    // contract is ever consulted.
    let sibling_projection = selections_of(case.sibling).project_psi();
    let foreign_rule = built_in_psi_steps_for_selections(sibling_projection.selections())
        .unwrap()
        .iter()
        .flat_map(crate::rules::PsiPassStep::ordered_rules)
        .find(|rule| *rule != (case.expected_rule)())
        .expect("the sibling schedule supplies a foreign rule identity");
    let mut run = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let commit = run.commits.first_mut().expect("positive run has a commit");
    commit.rule = foreign_rule;
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

/// Downstream replay: the run's recorded external decision log drives an
/// independent `replay_psi_pipeline` to the identical run surface — commits,
/// baseline and external decisions, manifests, ledger, and the transformed
/// unit — and the replayed run republishes the identical unit.
pub(super) fn assert_downstream_replay_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let baseline = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let replayed = replay_psi_pipeline(
        (case.positive)(),
        &selections,
        budget(64),
        &baseline.external_decisions().encode(),
    )
    .unwrap();
    assert_eq!(replayed.commits(), baseline.commits());
    assert_eq!(replayed.decisions(), baseline.decisions());
    assert_eq!(replayed.external_decisions(), baseline.external_decisions());
    assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
    assert_eq!(
        replayed.transformation_ledger(),
        baseline.transformation_ledger()
    );
    assert_eq!(
        replayed.session().unit().identity,
        baseline.session().unit().identity
    );
    let plan = publish_optimization_run(replayed).unwrap();
    assert_eq!(plan.unit().identity, baseline.session().unit().identity);
}

/// Stale evidence: a recorded external decision log replays only against the
/// exact context it was bound to. A log recorded against another workload
/// mismatches its source axis, one replayed under a sibling selection
/// mismatches its selections axis, and a forged foreign decision-target
/// identity mismatches the target axis — the closest executable wrong-target
/// surface a target-neutral phase owns. Each refuses before any candidate
/// action replays.
pub(super) fn assert_stale_evidence_leg(case: &SelectionMatrix) {
    let selections = selections_of(case.selection);
    let baseline = run_psi_pipeline((case.positive)(), &selections, budget(64)).unwrap();
    let encoded = baseline.external_decisions().encode();

    assert!(matches!(
        replay_psi_pipeline(verified_empty_unit(), &selections, budget(64), &encoded),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::ContextMismatch(ExternalDecisionContextAxis::Source)
        ))
    ));
    assert!(matches!(
        replay_psi_pipeline(
            (case.positive)(),
            &selections_of(case.sibling),
            budget(64),
            &encoded,
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::ContextMismatch(ExternalDecisionContextAxis::Selections)
        ))
    ));

    let context = baseline.external_decisions().context();
    let foreign_target = external_log_with(
        ExternalDecisionContext::new(
            context.schema(),
            context.source(),
            context.selections(),
            context.phase_selections(),
            optimization_core::OptimizationDecisionTargetIdentity::from_canonical_bytes(
                b"foreign target",
            ),
            context.rule_set(),
            context.cost_model(),
        ),
        baseline.external_decisions().points().iter().cloned(),
    );
    assert!(matches!(
        replay_psi_pipeline(
            (case.positive)(),
            &selections,
            budget(64),
            &foreign_target.encode()
        ),
        Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::ContextMismatch(ExternalDecisionContextAxis::Target)
        ))
    ));
}

/// Composition boundaries: a selection naming only foreign-phase members
/// projects to the empty Psi schedule and identity-publishes; a mixed suite
/// executes only its Psi members and publishes under the complete selection's
/// identity; a duplicate member refuses at selection construction; and a
/// multi-member request to the single-registry resolver refuses as an
/// unsupported combination (the relocation selection, which owns no registry,
/// refuses as an unsupported optimization on the same surface).
pub(super) fn assert_composition_leg(case: &SelectionMatrix) {
    // Wrong-target disposition: the phase is target-neutral, so a selection
    // owned entirely by another phase is out of this entrance's authority and
    // runs as the empty phase rather than executing foreign members.
    let foreign_only =
        OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate]).unwrap();
    let unit = (case.positive)();
    let input_identity = unit.unit().identity;
    let run = run_psi_pipeline(unit, &foreign_only, budget(64)).unwrap();
    assert!(run.psi_selections().is_empty());
    assert!(run.commits().is_empty());
    let plan = publish_optimization_run(run).unwrap();
    assert_eq!(plan.unit().identity, input_identity);
    assert!(plan.transformation_ledger().records().is_empty());

    // Mixed composition: the phase executes its own member under the
    // complete selection's custody, and the foreign member contributes no
    // scheduled step.
    let mixed = OptimizationSelections::new([
        case.selection,
        Optimization::SelectedIncomingU12ExactAddImmediate,
    ])
    .unwrap();
    let run = run_psi_pipeline((case.positive)(), &mixed, budget(64)).unwrap();
    assert_eq!(run.selections(), &mixed);
    assert_eq!(run.psi_selections(), &selections_of(case.selection));
    assert_eq!(run.commits().len(), case.expected_commits);
    assert!(
        run.commits()
            .iter()
            .any(|commit| commit.rule == (case.expected_rule)())
    );
    publish_optimization_run(run).unwrap();

    // Duplicate members refuse at selection construction.
    assert_eq!(
        OptimizationSelections::new([case.selection, case.selection]),
        Err(DuplicateOptimization(case.selection))
    );

    // A two-member Psi selection is an unsupported combination for the
    // single-registry resolver.
    let pair = OptimizationSelections::new([case.selection, case.sibling]).unwrap();
    assert!(matches!(
        built_in_psi_registry(&pair),
        Err(RuleRegistryError::UnsupportedOptimizationCombination)
            | Err(RuleRegistryError::UnsupportedOptimization(_))
    ));
}
