//! Applied-decision publication custody.

use super::*;
use omega_abstract_operations_optimizer::{OptimizationRun, PSI_PASS_CATALOG};
use omega_optimization_core::{
    AcceptedObligationFactIdentity, AnalysisKind, AnalysisSet, OptimizationCandidateIdentity,
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationFactReference,
    OptimizationPassIdentity, OptimizationPassManifestRecord, OptimizationReasonCode,
    OptimizationRuleSetIdentity, OptimizationUnitIdentity, OptimizationValidatorIdentity,
};
use omega_optimization_core::{
    BaselineDecisionLogBuilder, ExternalCandidateFeatures, ExternalDecisionAction,
    ExternalDecisionLog, ExternalDecisionPoint, ValidatedCandidateSummary,
};

type Fixture = fn() -> VerifiedPsiOptimizationUnit;

fn suite_cases() -> [(Optimization, Fixture); 6] {
    [
        (
            Optimization::SparseConditionalConstantPropagation,
            exact_add_verified,
        ),
        (
            Optimization::ControlFlowCleanup,
            unreachable_private_machine_verified,
        ),
        (
            Optimization::CopyPropagation,
            redundant_block_parameter_verified,
        ),
        (
            Optimization::GlobalValueNumbering,
            proof_certified_local_cse_verified,
        ),
        (
            Optimization::ProofCheckElision,
            live_exact_add_zero_verified,
        ),
        (
            Optimization::DeadPureScalarElimination,
            dead_scalar_literals_verified,
        ),
    ]
}

#[test]
fn every_psi_suite_replays_every_validated_candidate_declaration() {
    let cases = suite_cases();
    assert_eq!(
        cases.map(|(optimization, _)| optimization),
        PSI_PASS_CATALOG.map(|entry| Optimization::from(entry.optimization())),
        "every public Psi pass must have an Applied-evidence custody fixture"
    );
    for (optimization, fixture) in cases {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let optimized = publish_optimization_run(run(fixture(), selections)).unwrap();
        assert!(!optimized.commits().is_empty(), "{optimization:?}");
        assert_eq!(
            optimized.validated_candidates().len(),
            optimized
                .pass_manifests()
                .iter()
                .map(|manifest| manifest.decisions().len())
                .sum::<usize>(),
            "{optimization:?}"
        );
        assert_eq!(
            optimized
                .pass_manifests()
                .iter()
                .flat_map(|manifest| manifest.decisions())
                .filter(|decision| { decision.verdict() == OptimizationCandidateVerdict::Applied })
                .count(),
            optimized.commits().len(),
            "{optimization:?}"
        );
    }
}

#[test]
fn genuine_skipped_candidate_retains_and_replays_its_full_declaration() {
    let run = externally_skipped_sccp_run();
    assert!(run.commits().is_empty());
    assert_eq!(run.validated_candidates().len(), 1);
    let retained = &run.validated_candidates()[0];
    let decision = &run.pass_manifests()[0].decisions()[0];
    assert_eq!(retained.declaration().identity(), decision.candidate());
    assert_eq!(retained.declaration().input(), decision.input());
    assert_eq!(retained.declaration().rule(), decision.rule());
    assert_eq!(retained.validator(), decision.validator().unwrap());
    assert_eq!(
        decision.verdict(),
        OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable)
    );
    let projected = publish_optimization_run(run).unwrap();
    assert_eq!(projected.validated_candidates().len(), 1);
    assert!(projected.commits().is_empty());
}

#[test]
fn coordinated_skipped_evidence_corruption_rejects_against_retained_declaration() {
    let mut analyses_run = externally_skipped_sccp_run();
    let skipped = first_skipped(&analyses_run);
    let changed_analyses = skipped
        .analyses
        .union(AnalysisSet::new([AnalysisKind::RegisterLiveness]));
    replace_manifest_evidence(
        &mut analyses_run,
        skipped.candidate,
        changed_analyses,
        skipped.facts.clone(),
    );
    replace_external_features(
        &mut analyses_run,
        skipped.candidate,
        Some(changed_analyses),
        Some(skipped.facts),
        None,
    );
    assert_axis(
        publish_optimization_run(analyses_run),
        AppliedDecisionCustodyAxis::ConsumedAnalyses,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut facts_run = externally_skipped_sccp_run();
    let skipped = first_skipped(&facts_run);
    let mut changed_facts = skipped.facts.clone();
    changed_facts.push(OptimizationFactReference::AcceptedObligation(
        AcceptedObligationFactIdentity::from_bytes([0xfd; 32]),
    ));
    changed_facts.sort_unstable();
    changed_facts.dedup();
    replace_manifest_evidence(
        &mut facts_run,
        skipped.candidate,
        skipped.analyses,
        changed_facts.clone(),
    );
    replace_external_features(
        &mut facts_run,
        skipped.candidate,
        Some(skipped.analyses),
        Some(changed_facts),
        None,
    );
    assert_axis(
        publish_optimization_run(facts_run),
        AppliedDecisionCustodyAxis::ConsumedFacts,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut cost_run = externally_skipped_sccp_run();
    let skipped = first_skipped(&cost_run);
    let changed_cost = skipped.predicted_cost_delta - 1;
    replace_baseline_cost(&mut cost_run, skipped.candidate, changed_cost);
    replace_external_features(
        &mut cost_run,
        skipped.candidate,
        None,
        None,
        Some(changed_cost),
    );
    assert_axis(
        publish_optimization_run(cost_run),
        AppliedDecisionCustodyAxis::PredictedCostDelta,
        Optimization::SparseConditionalConstantPropagation,
    );
}

#[test]
fn skipped_declaration_roster_pass_validator_and_verdict_fail_closed() {
    let mut omitted = externally_skipped_sccp_run();
    omitted.validated_candidates.clear();
    assert_axis(
        publish_optimization_run(omitted),
        AppliedDecisionCustodyAxis::ValidatedRoster,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut duplicated = externally_skipped_sccp_run();
    duplicated
        .validated_candidates
        .push(duplicated.validated_candidates[0].clone());
    assert_axis(
        publish_optimization_run(duplicated),
        AppliedDecisionCustodyAxis::ValidatedRoster,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut wrong_pass = externally_skipped_sccp_run();
    wrong_pass.validated_candidates[0].pass =
        OptimizationPassIdentity::from_canonical_bytes(b"foreign-retained-pass");
    assert_axis(
        publish_optimization_run(wrong_pass),
        AppliedDecisionCustodyAxis::ValidatedPass,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut wrong_validator = externally_skipped_sccp_run();
    wrong_validator.validated_candidates[0].validator =
        OptimizationValidatorIdentity::from_canonical_bytes(b"foreign-retained-validator");
    assert_axis(
        publish_optimization_run(wrong_validator),
        AppliedDecisionCustodyAxis::Validator,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut wrong_verdict = externally_skipped_sccp_run();
    let decision = wrong_verdict.pass_manifests[0].decisions()[0].clone();
    replace_decision(
        &mut wrong_verdict,
        decision.candidate(),
        OptimizationDecisionRecord::new(
            decision.input(),
            decision.candidate(),
            decision.rule(),
            OptimizationCandidateVerdict::Applied,
            decision.consumed_analyses(),
            decision.consumed_facts().to_vec(),
            decision.validator(),
        )
        .unwrap(),
    );
    assert_axis(
        publish_optimization_run(wrong_verdict),
        AppliedDecisionCustodyAxis::AppliedRoster,
        Optimization::SparseConditionalConstantPropagation,
    );
}

#[test]
fn coordinated_manifest_and_external_evidence_corruption_fails_for_every_psi_suite() {
    for (optimization, fixture) in suite_cases() {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let mut analyses_run = run(fixture(), selections.clone());
        let applied = first_applied(&analyses_run);
        let changed_analyses = applied
            .analyses
            .union(AnalysisSet::new([AnalysisKind::RegisterLiveness]));
        replace_manifest_evidence(
            &mut analyses_run,
            applied.candidate,
            changed_analyses,
            applied.facts.clone(),
        );
        replace_external_features(
            &mut analyses_run,
            applied.candidate,
            Some(changed_analyses),
            Some(applied.facts),
            None,
        );
        assert_axis(
            publish_optimization_run(analyses_run),
            AppliedDecisionCustodyAxis::ConsumedAnalyses,
            optimization,
        );

        let mut facts_run = run(fixture(), selections);
        let applied = first_applied(&facts_run);
        let mut changed_facts = applied.facts.clone();
        changed_facts.push(OptimizationFactReference::AcceptedObligation(
            AcceptedObligationFactIdentity::from_bytes([0xfe; 32]),
        ));
        changed_facts.sort_unstable();
        changed_facts.dedup();
        replace_manifest_evidence(
            &mut facts_run,
            applied.candidate,
            applied.analyses,
            changed_facts.clone(),
        );
        replace_external_features(
            &mut facts_run,
            applied.candidate,
            Some(applied.analyses),
            Some(changed_facts),
            None,
        );
        assert_axis(
            publish_optimization_run(facts_run),
            AppliedDecisionCustodyAxis::ConsumedFacts,
            optimization,
        );
    }
}

#[test]
fn commit_and_coordinated_baseline_cost_corruption_fail_for_every_psi_suite() {
    for (optimization, fixture) in suite_cases() {
        let selections = OptimizationSelections::new([optimization]).unwrap();
        let mut commit_run = run(fixture(), selections.clone());
        commit_run.commits[0].predicted_cost_delta += 1;
        assert_axis(
            publish_optimization_run(commit_run),
            AppliedDecisionCustodyAxis::CommitPredictedCostDelta,
            optimization,
        );

        let mut policy_run = run(fixture(), selections);
        let candidate = first_applied(&policy_run).candidate;
        let changed_cost = policy_run.commits[0].predicted_cost_delta - 1;
        replace_baseline_cost(&mut policy_run, candidate, changed_cost);
        replace_external_features(&mut policy_run, candidate, None, None, Some(changed_cost));
        assert_axis(
            publish_optimization_run(policy_run),
            AppliedDecisionCustodyAxis::PredictedCostDelta,
            optimization,
        );
    }
}

#[test]
fn selected_pass_identity_is_bound_before_flattened_rule_accounting() {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let mut wrong_pass = run(exact_add_verified(), selections);
    let manifest = wrong_pass.pass_manifests[0].clone();
    wrong_pass.pass_manifests[0] = OptimizationPassManifestRecord::new(
        OptimizationPassIdentity::from_canonical_bytes(b"foreign-pass"),
        manifest.input(),
        manifest.output(),
        manifest.ordered_rule_set(),
        manifest.ordered_rules().to_vec(),
        manifest.decisions().to_vec(),
        manifest.work_usage(),
    )
    .unwrap();
    assert_axis(
        publish_optimization_run(wrong_pass),
        AppliedDecisionCustodyAxis::ManifestPass,
        Optimization::SparseConditionalConstantPropagation,
    );

    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let mut run = run(exact_add_verified(), selections);
    let manifest = run.pass_manifests[0].clone();
    let mut reordered_rules = manifest.ordered_rules().to_vec();
    reordered_rules.reverse();
    let reordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&reordered_rules)
        .expect("SCCP rule identities remain unique when reordered");
    run.pass_manifests[0] = OptimizationPassManifestRecord::new(
        manifest.pass(),
        manifest.input(),
        manifest.output(),
        reordered_rule_set,
        reordered_rules,
        manifest.decisions().to_vec(),
        manifest.work_usage(),
    )
    .unwrap();
    assert_axis(
        publish_optimization_run(run),
        AppliedDecisionCustodyAxis::ManifestRuleOrder,
        Optimization::SparseConditionalConstantPropagation,
    );
}

#[test]
fn applied_manifest_pairing_rejects_each_identity_axis_and_roster_drift() {
    let selection = || {
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap()
    };
    let fixture = || run(exact_add_verified(), selection());

    let mut wrong_input = fixture();
    let decision = first_applied_decision(&wrong_input);
    replace_decision(
        &mut wrong_input,
        decision.candidate(),
        OptimizationDecisionRecord::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"foreign-input"),
            decision.candidate(),
            decision.rule(),
            decision.verdict(),
            decision.consumed_analyses(),
            decision.consumed_facts().to_vec(),
            decision.validator(),
        )
        .unwrap(),
    );
    assert_axis(
        publish_optimization_run(wrong_input),
        AppliedDecisionCustodyAxis::Input,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut wrong_candidate = fixture();
    let decision = first_applied_decision(&wrong_candidate);
    let original_candidate = decision.candidate();
    replace_decision(
        &mut wrong_candidate,
        original_candidate,
        OptimizationDecisionRecord::new(
            decision.input(),
            OptimizationCandidateIdentity::from_canonical_bytes(b"foreign-candidate"),
            decision.rule(),
            decision.verdict(),
            decision.consumed_analyses(),
            decision.consumed_facts().to_vec(),
            decision.validator(),
        )
        .unwrap(),
    );
    assert_axis(
        publish_optimization_run(wrong_candidate),
        AppliedDecisionCustodyAxis::Candidate,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut wrong_rule = fixture();
    let decision = first_applied_decision(&wrong_rule);
    let replacement_rule = wrong_rule.pass_manifests[0]
        .ordered_rules()
        .iter()
        .copied()
        .find(|rule| *rule != decision.rule())
        .expect("SCCP schedules more than one rule");
    replace_decision(
        &mut wrong_rule,
        decision.candidate(),
        OptimizationDecisionRecord::new(
            decision.input(),
            decision.candidate(),
            replacement_rule,
            decision.verdict(),
            decision.consumed_analyses(),
            decision.consumed_facts().to_vec(),
            decision.validator(),
        )
        .unwrap(),
    );
    assert_axis(
        publish_optimization_run(wrong_rule),
        AppliedDecisionCustodyAxis::Rule,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut wrong_validator = fixture();
    let decision = first_applied_decision(&wrong_validator);
    replace_decision(
        &mut wrong_validator,
        decision.candidate(),
        OptimizationDecisionRecord::new(
            decision.input(),
            decision.candidate(),
            decision.rule(),
            decision.verdict(),
            decision.consumed_analyses(),
            decision.consumed_facts().to_vec(),
            Some(OptimizationValidatorIdentity::from_canonical_bytes(
                b"foreign-validator",
            )),
        )
        .unwrap(),
    );
    assert_axis(
        publish_optimization_run(wrong_validator),
        AppliedDecisionCustodyAxis::Validator,
        Optimization::SparseConditionalConstantPropagation,
    );

    let mut missing_applied = fixture();
    let decision = first_applied_decision(&missing_applied);
    replace_decision(
        &mut missing_applied,
        decision.candidate(),
        OptimizationDecisionRecord::new(
            decision.input(),
            decision.candidate(),
            decision.rule(),
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable),
            decision.consumed_analyses(),
            decision.consumed_facts().to_vec(),
            decision.validator(),
        )
        .unwrap(),
    );
    assert_axis(
        publish_optimization_run(missing_applied),
        AppliedDecisionCustodyAxis::AppliedRoster,
        Optimization::SparseConditionalConstantPropagation,
    );
}

#[derive(Clone)]
struct AppliedEvidence {
    candidate: OptimizationCandidateIdentity,
    analyses: AnalysisSet,
    facts: Vec<OptimizationFactReference>,
}

#[derive(Clone)]
struct SkippedEvidence {
    candidate: OptimizationCandidateIdentity,
    predicted_cost_delta: i64,
    analyses: AnalysisSet,
    facts: Vec<OptimizationFactReference>,
}

fn first_applied(run: &OptimizationRun) -> AppliedEvidence {
    let decision = first_applied_decision(run);
    AppliedEvidence {
        candidate: decision.candidate(),
        analyses: decision.consumed_analyses(),
        facts: decision.consumed_facts().to_vec(),
    }
}

fn first_applied_decision(run: &OptimizationRun) -> OptimizationDecisionRecord {
    run.pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .find(|decision| decision.verdict() == OptimizationCandidateVerdict::Applied)
        .expect("fixture applies at least one candidate")
        .clone()
}

fn first_skipped(run: &OptimizationRun) -> SkippedEvidence {
    let decision = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .find(|decision| matches!(decision.verdict(), OptimizationCandidateVerdict::Skipped(_)))
        .expect("fixture skips at least one validated candidate");
    let predicted_cost_delta = run
        .validated_candidates()
        .iter()
        .find(|retained| retained.declaration().identity() == decision.candidate())
        .unwrap()
        .declaration()
        .predicted_cost_delta();
    SkippedEvidence {
        candidate: decision.candidate(),
        predicted_cost_delta,
        analyses: decision.consumed_analyses(),
        facts: decision.consumed_facts().to_vec(),
    }
}

fn externally_skipped_sccp_run() -> OptimizationRun {
    let selections =
        OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation]).unwrap();
    let baseline = run_pipeline(exact_add_verified(), selections.clone());
    let [point] = baseline.external_decisions().points() else {
        panic!("exact-add fixture has one decision point")
    };
    let skipped = ExternalDecisionPoint::new(
        point.input(),
        point.rule(),
        point.legal_candidates().iter().cloned(),
        ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
    )
    .unwrap();
    let external =
        ExternalDecisionLog::new(baseline.external_decisions().context(), [skipped]).unwrap();
    replay_psi_pipeline(
        exact_add_verified(),
        &selections,
        work_budget(),
        &external.encode(),
    )
    .unwrap()
}

fn replace_manifest_evidence(
    run: &mut OptimizationRun,
    candidate: OptimizationCandidateIdentity,
    analyses: AnalysisSet,
    facts: Vec<OptimizationFactReference>,
) {
    for manifest in &mut run.pass_manifests {
        let Some(position) = manifest
            .decisions()
            .iter()
            .position(|decision| decision.candidate() == candidate)
        else {
            continue;
        };
        let mut decisions = manifest.decisions().to_vec();
        let old = &decisions[position];
        decisions[position] = OptimizationDecisionRecord::new(
            old.input(),
            old.candidate(),
            old.rule(),
            old.verdict(),
            analyses,
            facts,
            old.validator(),
        )
        .unwrap();
        *manifest = OptimizationPassManifestRecord::new(
            manifest.pass(),
            manifest.input(),
            manifest.output(),
            manifest.ordered_rule_set(),
            manifest.ordered_rules().to_vec(),
            decisions,
            manifest.work_usage(),
        )
        .unwrap();
        return;
    }
    panic!("candidate must occur in a pass manifest")
}

fn replace_decision(
    run: &mut OptimizationRun,
    candidate: OptimizationCandidateIdentity,
    replacement: OptimizationDecisionRecord,
) {
    for manifest in &mut run.pass_manifests {
        let Some(position) = manifest
            .decisions()
            .iter()
            .position(|decision| decision.candidate() == candidate)
        else {
            continue;
        };
        let mut decisions = manifest.decisions().to_vec();
        decisions[position] = replacement;
        *manifest = OptimizationPassManifestRecord::new(
            manifest.pass(),
            manifest.input(),
            manifest.output(),
            manifest.ordered_rule_set(),
            manifest.ordered_rules().to_vec(),
            decisions,
            manifest.work_usage(),
        )
        .unwrap();
        return;
    }
    panic!("candidate must occur in a pass manifest")
}

fn replace_baseline_cost(
    run: &mut OptimizationRun,
    candidate: OptimizationCandidateIdentity,
    changed_cost: i64,
) {
    let mut policy = BaselineDecisionLogBuilder::default();
    for record in &run.decisions.records {
        let considered = record.considered.iter().map(|summary| {
            if summary.candidate == candidate {
                ValidatedCandidateSummary {
                    candidate,
                    predicted_cost_delta: changed_cost,
                }
            } else {
                *summary
            }
        });
        policy
            .record_validated_outcome(record.input, considered, record.outcome)
            .unwrap();
    }
    run.decisions = policy.finish();
}

fn replace_external_features(
    run: &mut OptimizationRun,
    candidate: OptimizationCandidateIdentity,
    analyses: Option<AnalysisSet>,
    facts: Option<Vec<OptimizationFactReference>>,
    cost: Option<i64>,
) {
    let points = run
        .external_decisions
        .points()
        .iter()
        .map(|point| {
            let features = point
                .legal_candidates()
                .iter()
                .map(|feature| {
                    let summary = feature.summary();
                    if feature.candidate() != candidate {
                        return feature.clone();
                    }
                    ExternalCandidateFeatures::new(
                        ValidatedCandidateSummary {
                            candidate,
                            predicted_cost_delta: cost.unwrap_or(summary.predicted_cost_delta),
                        },
                        analyses.unwrap_or(feature.consumed_analyses()),
                        facts
                            .clone()
                            .unwrap_or_else(|| feature.consumed_facts().to_vec()),
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            ExternalDecisionPoint::new(point.input(), point.rule(), features, point.action())
                .unwrap()
        })
        .collect::<Vec<_>>();
    run.external_decisions = ExternalDecisionLog::new(run.external_decisions.context(), points)
        .expect("coordinated external log remains canonical");
}

fn assert_axis(
    result: Result<ValidatedOptimizedAbstractPlan, OptimizedAbstractProjectionError>,
    expected: AppliedDecisionCustodyAxis,
    optimization: Optimization,
) {
    let actual = result.as_ref().err();
    assert!(
        matches!(
            result,
            Err(OptimizedAbstractProjectionError::AppliedDecisionCustody {
                axis,
                ..
            }) if axis == expected
        ),
        "{optimization:?} must fail at {expected:?}, got {actual:?}"
    );
}
