use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{
    OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationExecutionPhase,
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationRuleSetIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget,
};
use omega_optimization_policy::{
    BaselineDecisionLog, BaselineDecisionOutcome, BaselinePolicy, ExternalDecisionAction,
    ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint, ValidatedCandidateSummary,
    external_psi_decision_schema_v1_identity, psi_target_neutral_decision_target_v1_identity,
};
use omega_optimization_unit::{
    PsiOptimizationUnit, PsiTransformationLedger, PsiTransformationRecord,
};
use omega_optimization_validation::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

use crate::{AnalysisManager, OrderedRuleRegistry, RuleAnalysisView};

use super::{
    ExternalDecisionContextAxis, ExternalDecisionReplayError, OptimizationRun,
    OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit,
    VerifiedPsiOptimizationSession, accounting::*, baseline_psi_cost_model_identity,
};

pub(super) fn run_registries(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(session, selections, registries, budget_per_pass, None)
}

pub(super) fn run_registries_with_external_decisions(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
    external_decisions: ExternalDecisionLog,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(
        session,
        selections,
        registries,
        budget_per_pass,
        Some(external_decisions),
    )
}

fn run_registries_inner(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
    supplied_external_decisions: Option<ExternalDecisionLog>,
) -> Result<OptimizationRun, OptimizationRunError> {
    let psi_selections = selections.for_phase(OptimizationExecutionPhase::Psi);
    let initial_identity = session.unit.identity;
    let psi = session.unit.psi;
    let fuel_schedule = session.unit.fuel_schedule;
    let ordered_rules = registries
        .iter()
        .flat_map(OrderedRuleRegistry::contracts)
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizationRunError::DuplicatePipelineRule)?;
    let expected_external_context = ExternalDecisionContext::new(
        external_psi_decision_schema_v1_identity(),
        initial_identity,
        selections.identity(),
        psi_selections.identity(),
        psi_target_neutral_decision_target_v1_identity(),
        ordered_rule_set,
        baseline_psi_cost_model_identity(),
    );
    let mut external_replay = supplied_external_decisions
        .as_ref()
        .map(|decisions| ExternalDecisionReplayCursor::new(decisions, expected_external_context))
        .transpose()
        .map_err(OptimizationRunError::ExternalDecisionReplay)?;
    let mut unit = session.unit;
    let mut commits = Vec::new();
    let mut usage = OptimizationRunUsage::default();
    let mut pass_logs = Vec::with_capacity(registries.len());
    let mut external_points = Vec::new();
    let mut pass_manifests = Vec::with_capacity(registries.len());
    for registry in registries {
        let (output, pass_commits, pass_usage, pass_decisions, pass_manifest, _pass_ledger) =
            run_unit_inner(unit, registry, budget_per_pass, external_replay.as_mut())?;
        unit = output;
        commits.extend(pass_commits);
        usage = add_usage(usage, pass_usage)?;
        let pass_manifest = pass_manifest.ok_or(OptimizationRunError::MissingPassManifest)?;
        let manifest_decisions = pass_manifest.decisions().iter().collect::<Vec<_>>();
        external_points.extend(external_points_from_manifest_decisions(
            &pass_decisions,
            &manifest_decisions,
        )?);
        pass_logs.push(pass_decisions);
        pass_manifests.push(pass_manifest);
    }
    let decisions = BaselineDecisionLog::concatenate(&pass_logs)
        .map_err(OptimizationRunError::DecisionLogReplay)?;
    let external_decisions = ExternalDecisionLog::new(expected_external_context, external_points)
        .map_err(OptimizationRunError::ExternalDecisionSchema)?;
    if let Some(replay) = external_replay.as_ref() {
        replay
            .require_exhausted()
            .map_err(OptimizationRunError::ExternalDecisionReplay)?;
    }
    if supplied_external_decisions
        .as_ref()
        .is_some_and(|supplied| supplied != &external_decisions)
    {
        return Err(OptimizationRunError::ExternalDecisionReplay(
            ExternalDecisionReplayError::ReconstructedLogMismatch,
        ));
    }
    let transformation_ledger = PsiTransformationLedger::new(
        psi,
        fuel_schedule,
        initial_identity,
        unit.identity,
        commits
            .iter()
            .map(|commit| PsiTransformationRecord {
                rule: commit.rule,
                candidate: commit.candidate,
                validator: commit.validator,
                input: commit.input,
                output: commit.output,
                pruned_machines: commit.pruned_machines.clone(),
                provenance: commit.provenance.clone(),
            })
            .collect(),
    )
    .map_err(OptimizationRunError::InvalidTransformationLedger)?;
    let identity_bundle = OptimizationIdentityBundle::new(
        selections.identity(),
        ordered_rule_set,
        baseline_psi_cost_model_identity(),
        Some(decisions.identity),
        None,
        transformation_ledger.identity(),
    );
    Ok(OptimizationRun {
        selections: selections.clone(),
        psi_selections,
        budget_per_pass,
        session: VerifiedPsiOptimizationSession {
            input: session.input,
            unit,
        },
        commits,
        usage,
        decisions,
        external_decisions,
        pass_manifests,
        transformation_ledger,
        identity_bundle,
    })
}

pub(super) struct ExternalDecisionReplayCursor<'log> {
    points: &'log [ExternalDecisionPoint],
    pub(super) next: usize,
}

impl<'log> ExternalDecisionReplayCursor<'log> {
    pub(super) fn new(
        decisions: &'log ExternalDecisionLog,
        expected_context: ExternalDecisionContext,
    ) -> Result<Self, ExternalDecisionReplayError> {
        if let Some(axis) = external_context_mismatch(expected_context, decisions.context()) {
            return Err(ExternalDecisionReplayError::ContextMismatch(axis));
        }
        let mut loci = BTreeSet::new();
        for point in decisions.points() {
            if !loci.insert((point.input(), point.rule())) {
                return Err(ExternalDecisionReplayError::DuplicateDecision {
                    input: point.input(),
                    rule: point.rule(),
                });
            }
        }
        Ok(Self {
            points: decisions.points(),
            next: 0,
        })
    }

    pub(super) fn choose(
        &mut self,
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
        candidates: &[ValidatedCandidateSummary],
    ) -> Result<BaselineDecisionOutcome, ExternalDecisionReplayError> {
        let ordinal = self.next;
        let point =
            self.points
                .get(ordinal)
                .ok_or(ExternalDecisionReplayError::MissingDecision {
                    ordinal,
                    input,
                    rule,
                })?;
        let mut legal_candidates = candidates.to_vec();
        legal_candidates.sort_by_key(|candidate| candidate.candidate);
        if point.input() != input
            || point.rule() != rule
            || point.legal_candidates() != legal_candidates
        {
            return Err(ExternalDecisionReplayError::IllegalDecision {
                ordinal,
                expected_input: input,
                expected_rule: rule,
            });
        }
        self.next += 1;
        Ok(match point.action() {
            ExternalDecisionAction::Choose(candidate) => BaselineDecisionOutcome::Choose(candidate),
            ExternalDecisionAction::Skip(reason) => BaselineDecisionOutcome::Skip(reason),
        })
    }

    fn require_exhausted(&self) -> Result<(), ExternalDecisionReplayError> {
        let remaining = self.points.len() - self.next;
        if remaining == 0 {
            Ok(())
        } else {
            Err(ExternalDecisionReplayError::LeftoverDecisions {
                first_unused: self.next,
                remaining,
            })
        }
    }
}

fn external_context_mismatch(
    expected: ExternalDecisionContext,
    supplied: ExternalDecisionContext,
) -> Option<ExternalDecisionContextAxis> {
    if expected.schema() != supplied.schema() {
        Some(ExternalDecisionContextAxis::Schema)
    } else if expected.source() != supplied.source() {
        Some(ExternalDecisionContextAxis::Source)
    } else if expected.selections() != supplied.selections() {
        Some(ExternalDecisionContextAxis::Selections)
    } else if expected.phase_selections() != supplied.phase_selections() {
        Some(ExternalDecisionContextAxis::PhaseSelections)
    } else if expected.target() != supplied.target() {
        Some(ExternalDecisionContextAxis::Target)
    } else if expected.rule_set() != supplied.rule_set() {
        Some(ExternalDecisionContextAxis::RuleSet)
    } else if expected.cost_model() != supplied.cost_model() {
        Some(ExternalDecisionContextAxis::CostModel)
    } else {
        None
    }
}

fn add_usage(
    left: OptimizationRunUsage,
    right: OptimizationRunUsage,
) -> Result<OptimizationRunUsage, OptimizationRunError> {
    Ok(OptimizationRunUsage {
        rule_evaluations: left
            .rule_evaluations
            .checked_add(right.rule_evaluations)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        candidates: left
            .candidates
            .checked_add(right.candidates)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        validation_steps: left
            .validation_steps
            .checked_add(right.validation_steps)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        commits: left
            .commits
            .checked_add(right.commits)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
        iterations: left
            .iterations
            .checked_add(right.iterations)
            .ok_or(OptimizationRunError::WorkUsageOverflow)?,
    })
}

fn external_points_from_manifest_decisions(
    decisions: &BaselineDecisionLog,
    manifest_decisions: &[&OptimizationDecisionRecord],
) -> Result<Vec<ExternalDecisionPoint>, OptimizationRunError> {
    let mut points = Vec::with_capacity(decisions.records.len());
    for record in &decisions.records {
        let mut rule = None;
        for candidate in &record.considered {
            let matching = manifest_decisions
                .iter()
                .copied()
                .filter(|decision| {
                    decision.input() == record.input && decision.candidate() == candidate.candidate
                })
                .collect::<Vec<_>>();
            let [decision] = matching.as_slice() else {
                return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
            };
            if let Some(expected) = rule {
                if decision.rule() != expected {
                    return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
                }
            } else {
                rule = Some(decision.rule());
            }
            let expected_verdict = match record.outcome {
                BaselineDecisionOutcome::Choose(chosen) if chosen == candidate.candidate => {
                    OptimizationCandidateVerdict::Applied
                }
                BaselineDecisionOutcome::Choose(_) => {
                    OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded)
                }
                BaselineDecisionOutcome::Skip(reason) => {
                    OptimizationCandidateVerdict::Skipped(reason)
                }
            };
            if decision.verdict() != expected_verdict {
                return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
            }
        }
        points.push(
            ExternalDecisionPoint::new(
                record.input,
                rule.ok_or(OptimizationRunError::ExternalDecisionManifestMismatch)?,
                record.considered.iter().copied(),
                record.outcome.into(),
            )
            .map_err(OptimizationRunError::ExternalDecisionSchema)?,
        );
    }
    Ok(points)
}

/// Reconstruct the recorded external policy surface from the ordinary
/// baseline log and independently validated pass manifests. This validator is
/// intentionally separate from recording and derives no field from the
/// recorded points.
pub fn validate_external_decision_recording(
    run: &OptimizationRun,
) -> Result<(), OptimizationRunError> {
    let ordered_rules = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.ordered_rules().iter().copied())
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizationRunError::DuplicatePipelineRule)?;
    let manifest_decisions = run
        .pass_manifests()
        .iter()
        .flat_map(|manifest| manifest.decisions())
        .collect::<Vec<_>>();
    let points = external_points_from_manifest_decisions(run.decisions(), &manifest_decisions)?;
    let expected = ExternalDecisionLog::new(
        ExternalDecisionContext::new(
            external_psi_decision_schema_v1_identity(),
            run.transformation_ledger().input(),
            run.selections().identity(),
            run.psi_selections().identity(),
            psi_target_neutral_decision_target_v1_identity(),
            ordered_rule_set,
            baseline_psi_cost_model_identity(),
        ),
        points,
    )
    .map_err(OptimizationRunError::ExternalDecisionSchema)?;
    let decoded = ExternalDecisionLog::decode(&run.external_decisions().encode())
        .map_err(OptimizationRunError::ExternalDecisionSchema)?;
    if decoded != expected {
        return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
    }
    Ok(())
}

type OptimizationRunOutput = (
    PsiOptimizationUnit,
    Vec<PsiOptimizationCommit>,
    OptimizationRunUsage,
    BaselineDecisionLog,
    Option<OptimizationPassManifestRecord>,
    PsiTransformationLedger,
);

#[cfg(test)]
pub(super) fn run_unit(
    unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    run_unit_inner(unit, registry, budget, None)
}

pub(super) fn run_unit_inner(
    mut unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
    mut external_replay: Option<&mut ExternalDecisionReplayCursor<'_>>,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    let mut analyses = AnalysisManager::new(&unit);
    let initial_identity = unit.identity;
    let psi = unit.psi;
    let fuel_schedule = unit.fuel_schedule;
    let mut usage = OptimizationRunUsage::default();
    let mut commits = Vec::new();
    let mut candidate_decisions = Vec::new();
    let mut seen_candidates = BTreeSet::new();
    let mut seen_revisions = BTreeMap::from([(unit.identity, 0)]);
    let mut dispatched = BTreeSet::new();
    let mut policy = BaselinePolicy::default();
    loop {
        charge(&mut usage.iterations, budget.iterations(), "iterations")?;
        let previous_measure = convergence_measure(&unit, registry);
        let mut chosen: Option<(
            omega_optimization_unit::PsiRewriteCandidate,
            ValidatedPsiRewrite,
        )> = None;
        for rule in registry.iter() {
            let contract = rule.contract();
            dispatched.insert(contract.identity());
            charge(
                &mut usage.rule_evaluations,
                budget.rule_evaluations(),
                "rule evaluations",
            )?;
            let products = analyses
                .require_all(&unit, contract.required_analyses())
                .map_err(OptimizationRunError::Analysis)?
                .into_iter()
                .cloned()
                .collect::<Vec<_>>();
            let candidates = rule
                .propose(&unit, RuleAnalysisView::new(&products))
                .map_err(|error| OptimizationRunError::Proposal {
                    rule: contract.identity(),
                    error,
                })?;
            for _ in &candidates {
                charge(&mut usage.candidates, budget.candidates(), "candidates")?;
            }
            let mut validated = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                if !seen_candidates.insert(candidate.identity()) {
                    return Err(OptimizationRunError::DuplicateCandidate(
                        candidate.identity(),
                    ));
                }
                charge(
                    &mut usage.validation_steps,
                    budget.validation_steps(),
                    "validation steps",
                )?;
                let output = validate_psi_rewrite_candidate(&unit, &candidate)
                    .map_err(OptimizationRunError::CandidateValidation)?;
                validated.push((candidate, output));
            }
            if validated.is_empty() {
                continue;
            }
            let summaries = validated
                .iter()
                .map(|(candidate, _)| ValidatedCandidateSummary {
                    candidate: candidate.identity(),
                    predicted_cost_delta: candidate.predicted_cost_delta(),
                })
                .collect::<Vec<_>>();
            let outcome = if let Some(replay) = external_replay.as_deref_mut() {
                let outcome = replay
                    .choose(unit.identity, contract.identity(), &summaries)
                    .map_err(OptimizationRunError::ExternalDecisionReplay)?;
                policy
                    .record_validated_outcome(unit.identity, summaries.iter().copied(), outcome)
                    .map_err(|error| {
                        OptimizationRunError::ExternalDecisionReplay(
                            ExternalDecisionReplayError::InvalidRecordedOutcome(error),
                        )
                    })?
            } else {
                policy.choose(unit.identity, summaries.iter().copied())
            };
            for (candidate, accepted) in &validated {
                let verdict = match outcome {
                    BaselineDecisionOutcome::Choose(chosen) if chosen == candidate.identity() => {
                        OptimizationCandidateVerdict::Applied
                    }
                    BaselineDecisionOutcome::Choose(_) => {
                        OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded)
                    }
                    BaselineDecisionOutcome::Skip(reason) => {
                        OptimizationCandidateVerdict::Skipped(reason)
                    }
                };
                candidate_decisions.push(
                    OptimizationDecisionRecord::new(
                        unit.identity,
                        candidate.identity(),
                        contract.identity(),
                        verdict,
                        contract.required_analyses(),
                        candidate.consumed_facts(),
                        Some(accepted.validator()),
                    )
                    .map_err(OptimizationRunError::InvalidManifest)?,
                );
            }
            match outcome {
                BaselineDecisionOutcome::Choose(identity) => {
                    let index = validated
                        .iter()
                        .position(|(candidate, _)| candidate.identity() == identity)
                        .ok_or(OptimizationRunError::PolicySelectionMissing(identity))?;
                    chosen = Some(validated.swap_remove(index));
                    break;
                }
                BaselineDecisionOutcome::Skip(_) => continue,
            }
        }
        let Some((candidate, validated)) = chosen else {
            if dispatched.len() != registry.len() {
                return Err(OptimizationRunError::RegistryCoverageMismatch);
            }
            let decisions = policy.finish();
            let pass_manifest = build_pass_manifest(
                registry,
                initial_identity,
                unit.identity,
                &commits,
                &candidate_decisions,
                usage,
            )?;
            let transformation_ledger = PsiTransformationLedger::new(
                psi,
                fuel_schedule,
                initial_identity,
                unit.identity,
                commits
                    .iter()
                    .map(|commit| PsiTransformationRecord {
                        rule: commit.rule,
                        candidate: commit.candidate,
                        validator: commit.validator,
                        input: commit.input,
                        output: commit.output,
                        pruned_machines: commit.pruned_machines.clone(),
                        provenance: commit.provenance.clone(),
                    })
                    .collect(),
            )
            .map_err(OptimizationRunError::InvalidTransformationLedger)?;
            return Ok((
                unit,
                commits,
                usage,
                decisions,
                pass_manifest,
                transformation_ledger,
            ));
        };
        let input_identity = unit.identity;
        let validator = validated.validator();
        let candidate_identity = validated.candidate();
        let provenance = validated.provenance().to_vec();
        let pruned_machines = candidate.patch_ref().pruned_machine_custody().to_vec();
        let next = validated.into_unit();
        register_revision(&mut seen_revisions, next.identity, usage.iterations)?;
        let current_measure = convergence_measure(&next, registry);
        if current_measure >= previous_measure {
            return Err(OptimizationRunError::NonDecreasingConvergenceMeasure {
                previous: previous_measure,
                current: current_measure,
            });
        }
        analyses
            .commit_revision(&next, candidate.invalidated_analyses(), true)
            .map_err(OptimizationRunError::Analysis)?;
        charge(&mut usage.commits, budget.commits(), "commits")?;
        commits.push(PsiOptimizationCommit {
            rule: candidate.rule(),
            candidate: candidate_identity,
            validator,
            input: input_identity,
            output: next.identity,
            predicted_cost_delta: candidate.predicted_cost_delta(),
            pruned_machines,
            provenance,
            declaration: candidate,
        });
        unit = next;
    }
}
