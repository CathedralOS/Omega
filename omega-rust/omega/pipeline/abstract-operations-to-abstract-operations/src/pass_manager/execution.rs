use std::collections::{BTreeMap, BTreeSet};

use optimization::PsiOptimizationSelections;
use optimization_core::{
    BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionOutcome, ExternalDecisionLog,
};
use optimization_core::{
    Optimization, OptimizationCandidateVerdict, OptimizationDecisionRecord,
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationReasonCode,
    OptimizationRuleContract, OptimizationRuleSetIdentity, OptimizationSelections,
    OptimizationUnitIdentity, OptimizationWorkBudget,
};
use optimization_unit::{PsiOptimizationUnit, PsiTransformationLedger, PsiTransformationRecord};
use optimization_validation::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

use crate::{AnalysisManager, OrderedRuleRegistry, RuleAnalysisView};

use super::{
    CandidateContractAxis, ExternalDecisionReplayError, OptimizationRun, OptimizationRunError,
    OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration,
    VerifiedPsiOptimizationSession,
    accounting::*,
    baseline::choose_baseline,
    baseline_psi_cost_model_identity,
    external_policy::{
        ExternalDecisionReplayCursor, expected_context, external_points_from_manifest_decisions,
        validated_candidate_features,
    },
};

pub(super) fn run_registries(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(
        session,
        selections,
        psi_selections,
        registries,
        budget_per_pass,
        None,
    )
}

pub(super) fn run_registries_with_external_decisions(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
    external_decisions: ExternalDecisionLog,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(
        session,
        selections,
        psi_selections,
        registries,
        budget_per_pass,
        Some(external_decisions),
    )
}

fn run_registries_inner(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    psi_selection: &PsiOptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
    supplied_external_decisions: Option<ExternalDecisionLog>,
) -> Result<OptimizationRun, OptimizationRunError> {
    let psi_selections = OptimizationSelections::new(
        psi_selection
            .as_slice()
            .iter()
            .copied()
            .map(Optimization::from),
    )
    .expect("the canonical Psi selection is duplicate-free");
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
    let expected_external_context = expected_context(
        initial_identity,
        selections,
        &psi_selections,
        ordered_rule_set,
    );
    let mut external_replay = supplied_external_decisions
        .as_ref()
        .map(|decisions| ExternalDecisionReplayCursor::new(decisions, expected_external_context))
        .transpose()
        .map_err(OptimizationRunError::ExternalDecisionReplay)?;
    let mut unit = session.unit;
    let mut commits = Vec::new();
    let mut validated_candidates = Vec::new();
    let mut usage = OptimizationRunUsage::default();
    let mut pass_logs = Vec::with_capacity(registries.len());
    let mut external_points = Vec::new();
    let mut pass_manifests = Vec::with_capacity(registries.len());
    for registry in registries {
        let (output, pass_commits, pass_usage, pass_decisions, pass_manifest, _pass_ledger) =
            run_unit_inner_with_retention(
                unit,
                registry,
                budget_per_pass,
                external_replay.as_mut(),
                &mut validated_candidates,
            )?;
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
    let session = VerifiedPsiOptimizationSession::from_transformed(session.input, unit)
        .map_err(OptimizationRunError::CandidateValidation)?;
    Ok(OptimizationRun {
        selections: selections.clone(),
        psi_selections,
        budget_per_pass,
        session,
        commits,
        validated_candidates,
        usage,
        decisions,
        external_decisions,
        pass_manifests,
        transformation_ledger,
        identity_bundle,
    })
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

#[cfg(test)]
pub(super) fn run_unit_inner(
    unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
    external_replay: Option<&mut ExternalDecisionReplayCursor<'_>>,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    run_unit_inner_with_retention(unit, registry, budget, external_replay, &mut Vec::new())
}

fn run_unit_inner_with_retention(
    mut unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
    mut external_replay: Option<&mut ExternalDecisionReplayCursor<'_>>,
    validated_candidates: &mut Vec<PsiValidatedCandidateDeclaration>,
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
    let mut recorded_decisions = BaselineDecisionLogBuilder::default();
    loop {
        charge(&mut usage.iterations, budget.iterations(), "iterations")?;
        let previous_measure = convergence_measure(&unit, registry);
        let mut chosen: Option<(optimization_unit::PsiRewriteCandidate, ValidatedPsiRewrite)> =
            None;
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
                validate_candidate_contract(&candidate, unit.identity, contract)?;
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
                validated_candidates.push(PsiValidatedCandidateDeclaration {
                    pass: contract.pass(),
                    declaration: candidate.clone(),
                    validator: output.validator(),
                });
                validated.push((candidate, output));
            }
            if validated.is_empty() {
                continue;
            }
            let external_features = validated
                .iter()
                .map(|(candidate, _)| validated_candidate_features(candidate, contract))
                .collect::<Result<Vec<_>, _>>()
                .map_err(OptimizationRunError::ExternalDecisionSchema)?;
            let summaries = external_features
                .iter()
                .map(|features| features.summary())
                .collect::<Vec<_>>();
            let outcome = if let Some(replay) = external_replay.as_deref_mut() {
                replay
                    .choose(unit.identity, contract.identity(), &external_features)
                    .map_err(OptimizationRunError::ExternalDecisionReplay)?
            } else {
                choose_baseline(&summaries)
            };
            recorded_decisions
                .record_validated_outcome(unit.identity, summaries.iter().copied(), outcome)
                .map_err(|error| {
                    OptimizationRunError::ExternalDecisionReplay(
                        ExternalDecisionReplayError::InvalidRecordedOutcome(error),
                    )
                })?;
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
            let decisions = recorded_decisions.finish();
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

fn validate_candidate_contract(
    candidate: &optimization_unit::PsiRewriteCandidate,
    input: OptimizationUnitIdentity,
    contract: OptimizationRuleContract,
) -> Result<(), OptimizationRunError> {
    let axis = if candidate.input() != input {
        Some(CandidateContractAxis::Input)
    } else if candidate.rule() != contract.identity() {
        Some(CandidateContractAxis::Rule)
    } else if candidate.required_analyses() != contract.required_analyses() {
        Some(CandidateContractAxis::RequiredAnalyses)
    } else if candidate.invalidated_analyses() != contract.invalidated_analyses() {
        Some(CandidateContractAxis::InvalidatedAnalyses)
    } else if candidate.safety_class() != contract.safety_class() {
        Some(CandidateContractAxis::SafetyClass)
    } else {
        None
    };
    if let Some(axis) = axis {
        Err(OptimizationRunError::CandidateContractMismatch {
            candidate: candidate.identity(),
            axis,
        })
    } else {
        Ok(())
    }
}
