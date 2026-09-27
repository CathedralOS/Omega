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
use optimization_unit::{
    PsiCandidateDeclaration, PsiOptimizationUnit, PsiTransformationLedger, PsiTransformationRecord,
};
use optimization_unit_semantics::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

use crate::ranked_rewrites::loop_invariant_scalar_motion::{
    self, apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
    validate_loop_invariant_scalar_motion,
};
use crate::rules::PsiPassStep;
use crate::{AnalysisManager, OrderedRuleRegistry};

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

pub(crate) fn run_registries(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelections,
    steps: &[PsiPassStep],
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(
        session,
        selections,
        psi_selections,
        steps,
        budget_per_pass,
        None,
    )
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn run_registries_with_external_decisions(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelections,
    steps: &[PsiPassStep],
    budget_per_pass: OptimizationWorkBudget,
    external_decisions: ExternalDecisionLog,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(
        session,
        selections,
        psi_selections,
        steps,
        budget_per_pass,
        Some(external_decisions),
    )
}

fn run_registries_inner(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    psi_selection: &PsiOptimizationSelections,
    steps: &[PsiPassStep],
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
    let ordered_rules = steps
        .iter()
        .flat_map(PsiPassStep::ordered_rules)
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
    let input = session.input.clone();
    let mut unit = session.unit;
    let mut commits = Vec::new();
    let mut validated_candidates = Vec::new();
    let mut usage = OptimizationRunUsage::default();
    let mut pass_logs = Vec::with_capacity(steps.len());
    let mut external_points = Vec::new();
    let mut pass_manifests = Vec::with_capacity(steps.len());
    for step in steps {
        let (output, pass_commits, pass_usage, pass_decisions, pass_manifest, _pass_ledger) =
            match step {
                PsiPassStep::RewriteRegistry(registry) => run_unit_inner_with_retention(
                    unit,
                    registry,
                    step,
                    budget_per_pass,
                    external_replay.as_mut(),
                    &mut validated_candidates,
                )?,
                PsiPassStep::LoopInvariantScalarMotion => {
                    let step_session =
                        VerifiedPsiOptimizationSession::from_transformed(input.clone(), unit)
                            .map_err(OptimizationRunError::CandidateValidation)?;
                    run_loop_invariant_scalar_motion_step(
                        step_session,
                        step,
                        budget_per_pass,
                        external_replay.as_mut(),
                        &mut validated_candidates,
                    )?
                }
            };
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
    let step = PsiPassStep::RewriteRegistry(registry.clone());
    run_unit_inner_with_retention(
        unit,
        registry,
        &step,
        budget,
        external_replay,
        &mut Vec::new(),
    )
}

fn run_unit_inner_with_retention(
    mut unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    step: &PsiPassStep,
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
        let mut chosen: Option<(PsiCandidateDeclaration, ValidatedPsiRewrite)> = None;
        let mut revision = analyses
            .bind_revision(&unit)
            .map_err(OptimizationRunError::Analysis)?;
        for rule in registry.iter() {
            let contract = rule.contract();
            dispatched.insert(contract.identity());
            charge(
                &mut usage.rule_evaluations,
                budget.rule_evaluations(),
                "rule evaluations",
            )?;
            let products = revision
                .require_all(contract.required_analyses())
                .map_err(OptimizationRunError::Analysis)?;
            let candidates =
                rule.propose(&unit, products)
                    .map_err(|error| OptimizationRunError::Proposal {
                        rule: contract.identity(),
                        error,
                    })?;
            for _ in &candidates {
                charge(&mut usage.candidates, budget.candidates(), "candidates")?;
            }
            let mut validated = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                let declaration = PsiCandidateDeclaration::Rewrite(candidate);
                validate_candidate_contract(&declaration, unit.identity, contract)?;
                if !seen_candidates.insert(declaration.identity()) {
                    return Err(OptimizationRunError::DuplicateCandidate(
                        declaration.identity(),
                    ));
                }
                charge(
                    &mut usage.validation_steps,
                    budget.validation_steps(),
                    "validation steps",
                )?;
                let Some(candidate) = declaration.rewrite() else {
                    return Err(OptimizationRunError::CandidateContractMismatch {
                        candidate: declaration.identity(),
                        axis: CandidateContractAxis::Rule,
                    });
                };
                let output = validate_psi_rewrite_candidate(&unit, candidate)
                    .map_err(OptimizationRunError::CandidateValidation)?;
                validated_candidates.push(PsiValidatedCandidateDeclaration {
                    pass: contract.pass(),
                    declaration: declaration.clone(),
                    validator: output.validator(),
                });
                validated.push((declaration, output));
            }
            if validated.is_empty() {
                continue;
            }
            let external_features = validated
                .iter()
                .map(|(declaration, _)| validated_candidate_features(declaration, contract))
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
            for (declaration, accepted) in &validated {
                let verdict = match outcome {
                    BaselineDecisionOutcome::Choose(chosen) if chosen == declaration.identity() => {
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
                        declaration.identity(),
                        contract.identity(),
                        verdict,
                        contract.required_analyses(),
                        declaration.consumed_facts(),
                        Some(accepted.validator()),
                    )
                    .map_err(OptimizationRunError::InvalidManifest)?,
                );
            }
            match outcome {
                BaselineDecisionOutcome::Choose(identity) => {
                    let index = validated
                        .iter()
                        .position(|(declaration, _)| declaration.identity() == identity)
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
                step,
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
        let pruned_machines = candidate.pruned_machine_custody().to_vec();
        let invalidated_analyses = candidate.invalidated_analyses();
        let rule = candidate.rule();
        let predicted_cost_delta = candidate.predicted_cost_delta();
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
            .commit_revision(&next, invalidated_analyses, true)
            .map_err(OptimizationRunError::Analysis)?;
        charge(&mut usage.commits, budget.commits(), "commits")?;
        commits.push(PsiOptimizationCommit {
            rule,
            candidate: candidate_identity,
            validator,
            input: input_identity,
            output: next.identity,
            predicted_cost_delta,
            pruned_machines,
            provenance,
            declaration: candidate,
        });
        unit = next;
    }
}

/// Execute the loop-invariant scalar-motion boundary as one scheduled pass:
/// propose every component relocation the session still admits, validate each
/// independently, let policy choose, apply atomically, and repeat until the
/// component custody reaches a fixed point. Commit, decision, manifest, and
/// ledger rows match the rewrite-registry shape exactly; only the
/// declaration is the relocation roster.
fn run_loop_invariant_scalar_motion_step(
    mut session: VerifiedPsiOptimizationSession,
    step: &PsiPassStep,
    budget: OptimizationWorkBudget,
    mut external_replay: Option<&mut ExternalDecisionReplayCursor<'_>>,
    validated_candidates: &mut Vec<PsiValidatedCandidateDeclaration>,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    let contract = loop_invariant_scalar_motion::contract();
    let initial_identity = session.unit().identity;
    let psi = session.unit().psi;
    let fuel_schedule = session.unit().fuel_schedule;
    let mut usage = OptimizationRunUsage::default();
    let mut commits = Vec::new();
    let mut candidate_decisions = Vec::new();
    let mut seen_candidates = BTreeSet::new();
    let mut seen_revisions = BTreeMap::from([(initial_identity, 0)]);
    let mut recorded_decisions = BaselineDecisionLogBuilder::default();
    loop {
        charge(&mut usage.iterations, budget.iterations(), "iterations")?;
        let previous_measure = loop_invariant_member_node_count(&session);
        charge(
            &mut usage.rule_evaluations,
            budget.rule_evaluations(),
            "rule evaluations",
        )?;
        let candidates = propose_loop_invariant_scalar_motion(&session, budget.candidates())
            .map_err(OptimizationRunError::LoopInvariantScalarMotion)?;
        let mut validated = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            charge(&mut usage.candidates, budget.candidates(), "candidates")?;
            let declaration = PsiCandidateDeclaration::LoopInvariantScalarMotion(candidate);
            validate_candidate_contract(&declaration, session.unit().identity, contract)?;
            if !seen_candidates.insert(declaration.identity()) {
                return Err(OptimizationRunError::DuplicateCandidate(
                    declaration.identity(),
                ));
            }
            charge(
                &mut usage.validation_steps,
                budget.validation_steps(),
                "validation steps",
            )?;
            let candidate = declaration
                .loop_invariant_scalar_motion()
                .expect("the boundary proposes relocation declarations");
            let accepted = validate_loop_invariant_scalar_motion(&session, candidate)
                .map_err(OptimizationRunError::LoopInvariantScalarMotion)?;
            validated_candidates.push(PsiValidatedCandidateDeclaration {
                pass: contract.pass(),
                declaration,
                validator: accepted.validator(),
            });
            validated.push(accepted);
        }
        if validated.is_empty() {
            return finish_loop_invariant_scalar_motion_step(
                step,
                psi,
                fuel_schedule,
                initial_identity,
                session,
                commits,
                candidate_decisions,
                usage,
                recorded_decisions,
            );
        }
        let external_features = validated
            .iter()
            .map(|accepted| {
                validated_candidate_features(
                    &PsiCandidateDeclaration::LoopInvariantScalarMotion(
                        accepted.candidate().clone(),
                    ),
                    contract,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(OptimizationRunError::ExternalDecisionSchema)?;
        let summaries = external_features
            .iter()
            .map(|features| features.summary())
            .collect::<Vec<_>>();
        let outcome = if let Some(replay) = external_replay.as_deref_mut() {
            replay
                .choose(
                    session.unit().identity,
                    contract.identity(),
                    &external_features,
                )
                .map_err(OptimizationRunError::ExternalDecisionReplay)?
        } else {
            choose_baseline(&summaries)
        };
        recorded_decisions
            .record_validated_outcome(session.unit().identity, summaries.iter().copied(), outcome)
            .map_err(|error| {
                OptimizationRunError::ExternalDecisionReplay(
                    ExternalDecisionReplayError::InvalidRecordedOutcome(error),
                )
            })?;
        for accepted in &validated {
            let candidate = accepted.candidate();
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
                    session.unit().identity,
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
        let BaselineDecisionOutcome::Choose(identity) = outcome else {
            // Policy declined the whole validated batch: like a skipped rule
            // dispatch this ends the pass — a declined relocation is never
            // re-proposed under the same revision, so the step converges here.
            return finish_loop_invariant_scalar_motion_step(
                step,
                psi,
                fuel_schedule,
                initial_identity,
                session,
                commits,
                candidate_decisions,
                usage,
                recorded_decisions,
            );
        };
        let index = validated
            .iter()
            .position(|accepted| accepted.candidate().identity() == identity)
            .ok_or(OptimizationRunError::PolicySelectionMissing(identity))?;
        let accepted = validated.swap_remove(index);
        let declaration =
            PsiCandidateDeclaration::LoopInvariantScalarMotion(accepted.candidate().clone());
        let input_identity = session.unit().identity;
        let validator = accepted.validator();
        let candidate_identity = accepted.candidate().identity();
        let predicted_cost_delta = accepted.candidate().predicted_cost_delta();
        let provenance = accepted.provenance().to_vec();
        let applied = apply_loop_invariant_scalar_motion(session, accepted)
            .map_err(OptimizationRunError::LoopInvariantScalarMotion)?;
        // The per-commit ledger is consumed here; its single record binds the
        // same rule, candidate, validator, input, output, and provenance rows
        // the commit below chains into the pass-level transformation ledger.
        let (next_session, _, _) = applied.into_parts();
        session = next_session;
        let next_identity = session.unit().identity;
        register_revision(&mut seen_revisions, next_identity, usage.iterations)?;
        let current_measure = loop_invariant_member_node_count(&session);
        if current_measure >= previous_measure {
            return Err(OptimizationRunError::NonDecreasingConvergenceMeasure {
                previous: previous_measure,
                current: current_measure,
            });
        }
        charge(&mut usage.commits, budget.commits(), "commits")?;
        commits.push(PsiOptimizationCommit {
            rule: contract.identity(),
            candidate: candidate_identity,
            validator,
            input: input_identity,
            output: next_identity,
            predicted_cost_delta,
            pruned_machines: Vec::new(),
            provenance,
            declaration,
        });
    }
}

/// Seal the relocation step: finish the baseline decision log, build the
/// pass manifest, chain the step-local transformation ledger, and release
/// the session back to the caller's unit custody.
fn finish_loop_invariant_scalar_motion_step(
    step: &PsiPassStep,
    psi: terminal_psi::TerminalPsiIdentity,
    fuel_schedule: terminal_fuel::FuelScheduleIdentity,
    initial_identity: OptimizationUnitIdentity,
    session: VerifiedPsiOptimizationSession,
    commits: Vec<PsiOptimizationCommit>,
    candidate_decisions: Vec<OptimizationDecisionRecord>,
    usage: OptimizationRunUsage,
    recorded_decisions: BaselineDecisionLogBuilder,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    let decisions = recorded_decisions.finish();
    let pass_manifest = build_pass_manifest(
        step,
        initial_identity,
        session.unit().identity,
        &commits,
        &candidate_decisions,
        usage,
    )?;
    let transformation_ledger = PsiTransformationLedger::new(
        psi,
        fuel_schedule,
        initial_identity,
        session.unit().identity,
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
    let (_, unit) = session.into_parts();
    Ok((
        unit,
        commits,
        usage,
        decisions,
        pass_manifest,
        transformation_ledger,
    ))
}

/// The relocation step's convergence measure: the number of nodes still
/// inside authenticated cyclic-component member blocks. Every accepted
/// candidate moves its roster to the unique preheader, so the count strictly
/// decreases across a commit.
fn loop_invariant_member_node_count(session: &VerifiedPsiOptimizationSession) -> u64 {
    session
        .cycle_components()
        .components()
        .iter()
        .map(|component| {
            session
                .unit()
                .functions
                .iter()
                .filter(|function| function.machine == component.id.machine)
                .flat_map(|function| function.blocks.iter())
                .filter(|block| component.members.contains(&block.id))
                .map(|block| u64::try_from(block.nodes.len()).expect("block node count fits u64"))
                .sum::<u64>()
        })
        .sum()
}

fn validate_candidate_contract(
    candidate: &PsiCandidateDeclaration,
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
