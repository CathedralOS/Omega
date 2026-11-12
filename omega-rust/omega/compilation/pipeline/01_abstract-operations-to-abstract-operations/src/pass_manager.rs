/*
Deterministic fixed-point execution of selected Psi optimization passes
(`VerifiedPsiOptimizationSession`, `OptimizationRun`, `run_registries`),
per-pass work-budget and manifest accounting, baseline candidate selection,
and external decision-policy feature extraction, recording, and replay.
*/

pub(crate) mod accounting {
    use std::collections::BTreeMap;

    use abstract_operations::{AbstractOperation as O, IntegerShiftPolicy};
    use optimization_core::{
        OptimizationCandidateVerdict, OptimizationDecisionRecord, OptimizationPassIdentity,
        OptimizationPassManifestRecord, OptimizationUnitIdentity, OptimizationWorkUsage,
    };
    use optimization_unit::PsiOptimizationUnit;

    use crate::OrderedRuleRegistry;

    use super::{OptimizationRunError, OptimizationRunUsage, PsiOptimizationCommit};

    pub(crate) fn register_revision(
        seen: &mut BTreeMap<OptimizationUnitIdentity, u64>,
        identity: OptimizationUnitIdentity,
        iteration: u64,
    ) -> Result<(), OptimizationRunError> {
        if let Some(first_seen_iteration) = seen.get(&identity).copied() {
            return Err(OptimizationRunError::OscillatingRevision {
                identity,
                first_seen_iteration,
                repeated_at_iteration: iteration,
            });
        }
        seen.insert(identity, iteration);
        Ok(())
    }

    pub(crate) fn build_pass_manifest(
        step: &crate::rules::PsiPassStep,
        input: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        commits: &[PsiOptimizationCommit],
        decisions: &[OptimizationDecisionRecord],
        usage: OptimizationRunUsage,
    ) -> Result<Option<OptimizationPassManifestRecord>, OptimizationRunError> {
        let Some(pass) = step.pass() else {
            return Ok(None);
        };
        let ordered_rules = step.ordered_rules();
        for commit in commits {
            assert!(
                decisions.iter().any(|decision| {
                    decision.candidate() == commit.candidate
                        && decision.verdict() == OptimizationCandidateVerdict::Applied
                }),
                "every committed candidate has an applied manifest decision"
            );
        }
        OptimizationPassManifestRecord::new(
            pass,
            input,
            output,
            step.rule_set_identity(),
            ordered_rules,
            decisions.to_vec(),
            OptimizationWorkUsage {
                rule_evaluations: usage.rule_evaluations,
                candidates: usage.candidates,
                validation_steps: usage.validation_steps,
                commits: usage.commits,
                iterations: usage.iterations,
            },
        )
        .map(Some)
        .map_err(OptimizationRunError::InvalidManifest)
    }

    pub(crate) fn charge(
        counter: &mut u64,
        limit: u64,
        axis: &'static str,
    ) -> Result<(), OptimizationRunError> {
        if *counter == limit {
            return Err(OptimizationRunError::WorkBudgetExhausted(axis));
        }
        *counter += 1;
        Ok(())
    }

    pub(crate) fn add_usage(
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

    fn integer_evaluation_operation_count(unit: &PsiOptimizationUnit) -> u64 {
        unit.block_nodes()
            .filter(|node| {
                matches!(
                    node.operation,
                    O::IntegerArithmetic { .. }
                        | O::IntegerShift { .. }
                        | O::IntegerExactCast { .. }
                        | O::IntegerWiden { .. }
                        | O::IntegerBitwiseNot { .. }
                        | O::IntegerBitwiseAnd { .. }
                        | O::IntegerBitwiseOr { .. }
                        | O::IntegerBitwiseXor { .. }
                        | O::BooleanNot { .. }
                        | O::BooleanEqual { .. }
                        | O::IntegerEqual { .. }
                        | O::IntegerLessThan { .. }
                        | O::IntegerLessOrEqual { .. }
                )
            })
            .count()
            .try_into()
            .expect("operation count fits u64")
    }

    fn block_parameter_count(unit: &PsiOptimizationUnit) -> u64 {
        unit.blocks()
            .map(|block| u64::try_from(block.parameters.len()).expect("parameter count fits u64"))
            .sum()
    }

    fn dead_total_scalar_operation_count(unit: &PsiOptimizationUnit) -> u64 {
        unit.block_nodes()
            .filter(|node| match &node.operation {
                O::IntegerArithmetic { arithmetic, .. } => arithmetic.obligation().is_none(),
                operation => matches!(
                    operation,
                    O::IntegerConstant { .. }
                        | O::BooleanConstant { .. }
                        | O::BooleanNot { .. }
                        | O::BooleanEqual { .. }
                        | O::IntegerEqual { .. }
                        | O::IntegerLessThan { .. }
                        | O::IntegerLessOrEqual { .. }
                        | O::IntegerBitwiseNot { .. }
                        | O::IntegerBitwiseAnd { .. }
                        | O::IntegerBitwiseOr { .. }
                        | O::IntegerBitwiseXor { .. }
                        | O::IntegerWiden { .. }
                        | O::IntegerShift {
                            policy: IntegerShiftPolicy::Wrapping,
                            ..
                        }
                ),
            })
            .count()
            .try_into()
            .expect("dead-total scalar operation count fits u64")
    }

    fn proof_certified_scalar_operation_count(unit: &PsiOptimizationUnit) -> u64 {
        unit.block_nodes()
            .filter(|node| match &node.operation {
                O::IntegerArithmetic { arithmetic, .. } => arithmetic.obligation().is_some(),
                operation => matches!(
                    operation,
                    O::IntegerExactCast { .. }
                        | O::IntegerShift {
                            policy: IntegerShiftPolicy::Exact { .. },
                            ..
                        }
                ),
            })
            .count()
            .try_into()
            .expect("proof-certified scalar operation count fits u64")
    }

    fn control_flow_structure_count(unit: &PsiOptimizationUnit) -> u64 {
        unit.functions
            .iter()
            .map(|function| {
                1 + u64::try_from(function.blocks.len()).expect("block count fits u64")
                    + function
                        .blocks
                        .iter()
                        .map(|block| {
                            u64::try_from(block.nodes.len()).expect("node count fits u64")
                                + block
                                    .nodes
                                    .iter()
                                    .map(|node| {
                                        u64::try_from(node.successors.len())
                                            .expect("successor count fits u64")
                                    })
                                    .sum::<u64>()
                        })
                        .sum::<u64>()
            })
            .sum()
    }

    pub(crate) fn convergence_measure(
        unit: &PsiOptimizationUnit,
        registry: &OrderedRuleRegistry,
    ) -> u64 {
        let copy_pass =
            OptimizationPassIdentity::from_canonical_bytes(b"omega.psi-pass.copy-propagation.v1");
        let cfg_pass = OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.control-flow-cleanup.v13",
        );
        let dead_scalar_pass = OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.dead-pure-scalar-elimination.v2",
        );
        let proof_elision_pass = OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.proof-check-elision.v12",
        );
        let global_value_numbering_pass = OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.global-value-numbering.v14",
        );
        let state_specialization_pass = OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.state-specialization.v1",
        );
        let representation_specialization_pass = OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.representation-specialization.v1",
        );
        if registry.pass() == Some(cfg_pass) {
            control_flow_structure_count(unit)
        } else if registry.pass() == Some(copy_pass) {
            block_parameter_count(unit)
        } else if registry.pass() == Some(dead_scalar_pass) {
            dead_total_scalar_operation_count(unit)
        } else if registry.pass() == Some(proof_elision_pass) {
            proof_certified_scalar_operation_count(unit)
        } else if registry.pass() == Some(global_value_numbering_pass) {
            unit.blocks().map(|block| block.nodes.len() as u64).sum()
        } else if registry.pass() == Some(state_specialization_pass) {
            dispatch_chain_depth_measure(unit)
        } else if registry.pass() == Some(representation_specialization_pass) {
            representation_observation_count(unit)
        } else {
            integer_evaluation_operation_count(unit)
        }
    }

    fn representation_observation_count(unit: &PsiOptimizationUnit) -> u64 {
        unit.block_nodes()
            .filter(|node| {
                matches!(
                    node.operation,
                    O::StructuralCaseMembership { .. } | O::StructuralScalarFieldRead { .. }
                )
            })
            .count() as u64
    }

    fn dispatch_chain_depth_measure(unit: &PsiOptimizationUnit) -> u64 {
        use std::collections::BTreeSet;

        use abstract_operations::AbstractOperation;
        use optimization_unit::PsiOptimizationFunction;
        use semantic_vocabulary::BlockId;

        fn depth(
            function: &PsiOptimizationFunction,
            block: BlockId,
            memo: &mut BTreeMap<BlockId, u64>,
            visiting: &mut BTreeSet<BlockId>,
        ) -> u64 {
            if let Some(cached) = memo.get(&block) {
                return *cached;
            }
            if !visiting.insert(block) {
                return 0;
            }
            let resolved = function
                .block(block)
                .and_then(|owner| {
                    let node = owner.nodes.last()?;
                    match &node.operation {
                        AbstractOperation::Conditional { .. } => Some(
                            node.successors
                                .iter()
                                .map(|edge| depth(function, edge.target, memo, visiting))
                                .max()
                                .unwrap_or(0)
                                + 1,
                        ),
                        _ => None,
                    }
                })
                .unwrap_or(0);
            visiting.remove(&block);
            memo.insert(block, resolved);
            resolved
        }

        unit.functions
            .iter()
            .map(|function| {
                let mut memo = BTreeMap::new();
                let mut visiting = BTreeSet::new();
                function
                    .block_edges()
                    .map(|edge| depth(function, edge.target, &mut memo, &mut visiting))
                    .sum::<u64>()
            })
            .sum()
    }
}
pub(crate) mod baseline {
    use optimization_core::{
        BaselineDecisionOutcome, OptimizationReasonCode, ValidatedCandidateSummary,
    };

    pub(crate) fn choose_baseline(
        candidates: &[ValidatedCandidateSummary],
    ) -> BaselineDecisionOutcome {
        candidates
            .iter()
            .min_by_key(|candidate| (candidate.predicted_cost_delta, candidate.candidate))
            .filter(|candidate| candidate.predicted_cost_delta < 0)
            .map_or(
                BaselineDecisionOutcome::Skip(OptimizationReasonCode::NotProfitable),
                |candidate| BaselineDecisionOutcome::Choose(candidate.candidate),
            )
    }
}
pub(crate) mod execution {
    use std::collections::{BTreeMap, BTreeSet};

    use optimization::PsiOptimizationSelections;
    use optimization_core::{
        BaselineDecisionLog, BaselineDecisionLogBuilder, BaselineDecisionOutcome,
        ExternalDecisionLog,
    };
    use optimization_core::{
        Optimization, OptimizationDecisionRecord, OptimizationIdentityBundle,
        OptimizationPassManifestRecord, OptimizationRuleContract, OptimizationRuleSetIdentity,
        OptimizationSelections, OptimizationUnitIdentity, OptimizationWorkBudget,
    };
    use optimization_unit::{
        PsiCandidateDeclaration, PsiOptimizationUnit, PsiTransformationLedger,
        PsiTransformationRecord,
    };
    use optimization_unit_semantics::{ValidatedPsiRewrite, validate_psi_rewrite_candidate};

    use crate::ranked_rewrites::loop_invariant_scalar_motion::{
        self, apply_loop_invariant_scalar_motion, propose_loop_invariant_scalar_motion,
        validate_loop_invariant_scalar_motion,
    };
    use crate::rules::PsiPassStep;
    use crate::{AnalysisRevision, OrderedRuleRegistry};

    use super::{
        CandidateContractAxis, ExternalDecisionReplayError, OptimizationRun, OptimizationRunError,
        OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration,
        VerifiedPsiOptimizationSession,
        accounting::*,
        baseline::choose_baseline,
        baseline_outcome_verdict, baseline_psi_cost_model_identity,
        external_policy::{
            ExternalDecisionReplayCursor, expected_context,
            external_points_from_manifest_decisions, validated_candidate_features,
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
        let work_schedule = session.unit.work_schedule;
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
            .map(|decisions| {
                ExternalDecisionReplayCursor::new(decisions, expected_external_context)
            })
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
        let external_decisions =
            ExternalDecisionLog::new(expected_external_context, external_points)
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
        let output = unit.identity;
        let ledger = chain_ledger(psi, work_schedule, initial_identity, output, &commits)?;
        let identity_bundle = OptimizationIdentityBundle::new(
            selections.identity(),
            ordered_rule_set,
            baseline_psi_cost_model_identity(),
            Some(decisions.identity),
            None,
            ledger.identity(),
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
            transformation_ledger: ledger,
            identity_bundle,
        })
    }

    /// Census for `check_on_lowered_form.md` and `stage_trust.md`: committed
    /// rewrites in one pass step that consumed an accepted obligation fact, by
    /// rule. Such a rewrite removes or folds a check on the strength of that
    /// fact, so the count is how many checks would return if a fact without
    /// checked evidence had to keep its check. Rule identities are digests of
    /// the rule names in `proof_check_elision.rs` and `rules.rs`; the first
    /// eight bytes are printed.
    fn accepted_obligation_rewrite_census(commits: &[PsiOptimizationCommit]) -> String {
        let mut rules: Vec<([u8; 8], usize)> = Vec::new();
        for commit in commits {
            let consumes = commit.declaration.consumed_facts().iter().any(|fact| {
                matches!(
                    fact,
                    optimization_core::OptimizationFactReference::AcceptedObligation(_)
                )
            });
            if !consumes {
                continue;
            }
            let mut prefix = [0; 8];
            prefix.copy_from_slice(&commit.rule.bytes()[..8]);
            match rules.iter_mut().find(|(rule, _)| *rule == prefix) {
                Some((_, count)) => *count += 1,
                None => rules.push((prefix, 1)),
            }
        }
        let consuming: usize = rules.iter().map(|(_, count)| count).sum();
        let mut census = format!(
            "accepted_obligation_rewrite_step commits={} consuming={consuming}\n",
            commits.len()
        );
        for (rule, count) in &rules {
            let hex: String = rule.iter().map(|byte| format!("{byte:02x}")).collect();
            census.push_str(&format!(
                "accepted_obligation_rewrite rule={hex} commits={count}\n"
            ));
        }
        census
    }

    type OptimizationRunOutput = (
        PsiOptimizationUnit,
        Vec<PsiOptimizationCommit>,
        OptimizationRunUsage,
        BaselineDecisionLog,
        Option<OptimizationPassManifestRecord>,
        PsiTransformationLedger,
    );

    fn run_unit_inner_with_retention(
        mut unit: PsiOptimizationUnit,
        registry: &OrderedRuleRegistry,
        step: &PsiPassStep,
        budget: OptimizationWorkBudget,
        mut external_replay: Option<&mut ExternalDecisionReplayCursor<'_>>,
        validated_candidates: &mut Vec<PsiValidatedCandidateDeclaration>,
    ) -> Result<OptimizationRunOutput, OptimizationRunError> {
        let initial_identity = unit.identity;
        let psi = unit.psi;
        let work_schedule = unit.work_schedule;
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
            let mut revision =
                AnalysisRevision::bind(&unit).map_err(OptimizationRunError::Analysis)?;
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
                let candidates = rule.propose(&unit, products).map_err(|error| {
                    OptimizationRunError::Proposal {
                        rule: contract.identity(),
                        error,
                    }
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
                    let verdict = baseline_outcome_verdict(outcome, declaration.identity());
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
                if std::env::var_os("OMEGA_PROOF_MEASUREMENTS").is_some() {
                    eprint!("{}", accepted_obligation_rewrite_census(&commits));
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
                let output = unit.identity;
                let ledger = chain_ledger(psi, work_schedule, initial_identity, output, &commits)?;
                return Ok((unit, commits, usage, decisions, pass_manifest, ledger));
            };
            let input_identity = unit.identity;
            let validator = validated.validator();
            let candidate_identity = validated.candidate();
            let provenance = validated.provenance().to_vec();
            let pruned_machines = candidate.pruned_machine_custody().to_vec();
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
        let work_schedule = session.unit().work_schedule;
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
                    work_schedule,
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
                .record_validated_outcome(
                    session.unit().identity,
                    summaries.iter().copied(),
                    outcome,
                )
                .map_err(|error| {
                    OptimizationRunError::ExternalDecisionReplay(
                        ExternalDecisionReplayError::InvalidRecordedOutcome(error),
                    )
                })?;
            for accepted in &validated {
                let candidate = accepted.candidate();
                let verdict = baseline_outcome_verdict(outcome, candidate.identity());
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
                return finish_loop_invariant_scalar_motion_step(
                    step,
                    psi,
                    work_schedule,
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

    fn chain_ledger(
        psi: terminal_psi::TerminalPsiIdentity,
        work_schedule: psi_work::WorkScheduleIdentity,
        initial_identity: OptimizationUnitIdentity,
        output: OptimizationUnitIdentity,
        commits: &[PsiOptimizationCommit],
    ) -> Result<PsiTransformationLedger, OptimizationRunError> {
        PsiTransformationLedger::new(
            psi,
            work_schedule,
            initial_identity,
            output,
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
        .map_err(OptimizationRunError::InvalidTransformationLedger)
    }

    fn finish_loop_invariant_scalar_motion_step(
        step: &PsiPassStep,
        psi: terminal_psi::TerminalPsiIdentity,
        work_schedule: psi_work::WorkScheduleIdentity,
        initial_identity: OptimizationUnitIdentity,
        session: VerifiedPsiOptimizationSession,
        commits: Vec<PsiOptimizationCommit>,
        candidate_decisions: Vec<OptimizationDecisionRecord>,
        usage: OptimizationRunUsage,
        recorded_decisions: BaselineDecisionLogBuilder,
    ) -> Result<OptimizationRunOutput, OptimizationRunError> {
        let decisions = recorded_decisions.finish();
        let output = session.unit().identity;
        let pass_manifest = build_pass_manifest(
            step,
            initial_identity,
            output,
            &commits,
            &candidate_decisions,
            usage,
        )?;
        let ledger = chain_ledger(psi, work_schedule, initial_identity, output, &commits)?;
        let (_, unit) = session.into_parts();
        Ok((unit, commits, usage, decisions, pass_manifest, ledger))
    }

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
                    .map(|block| {
                        u64::try_from(block.nodes.len()).expect("block node count fits u64")
                    })
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
}
pub(crate) mod external_policy {
    pub(crate) mod candidate_features {
        use optimization_core::OptimizationRuleContract;
        use optimization_core::{
            ExternalCandidateFeatures, ExternalDecisionSchemaError, ValidatedCandidateSummary,
        };
        use optimization_unit::PsiCandidateDeclaration;

        pub(crate) fn derive(
            candidate: &PsiCandidateDeclaration,
            contract: OptimizationRuleContract,
        ) -> Result<ExternalCandidateFeatures, ExternalDecisionSchemaError> {
            ExternalCandidateFeatures::new(
                ValidatedCandidateSummary {
                    candidate: candidate.identity(),
                    predicted_cost_delta: candidate.predicted_cost_delta(),
                },
                contract.required_analyses(),
                candidate.consumed_facts(),
            )
        }
    }
    pub(crate) mod context {
        use optimization_core::{
            ExternalDecisionContext, external_psi_decision_schema_identity,
            psi_target_neutral_decision_target_identity,
        };
        use optimization_core::{
            OptimizationRuleSetIdentity, OptimizationSelections, OptimizationUnitIdentity,
        };

        use super::super::{ExternalDecisionContextAxis, baseline_psi_cost_model_identity};

        pub(crate) fn expected_context(
            source: OptimizationUnitIdentity,
            selections: &OptimizationSelections,
            phase_selections: &OptimizationSelections,
            rule_set: OptimizationRuleSetIdentity,
        ) -> ExternalDecisionContext {
            ExternalDecisionContext::new(
                external_psi_decision_schema_identity(),
                source,
                selections.identity(),
                phase_selections.identity(),
                psi_target_neutral_decision_target_identity(),
                rule_set,
                baseline_psi_cost_model_identity(),
            )
        }

        pub(crate) fn mismatch(
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
    }
    pub(crate) mod recording {
        use optimization_core::{
            BaselineDecisionLog, ExternalCandidateFeatures, ExternalDecisionPoint,
            OptimizationDecisionRecord,
        };

        use super::super::{OptimizationRunError, baseline_outcome_verdict};

        pub(crate) fn external_points_from_manifest_decisions(
            decisions: &BaselineDecisionLog,
            manifest_decisions: &[&OptimizationDecisionRecord],
        ) -> Result<Vec<ExternalDecisionPoint>, OptimizationRunError> {
            let mut points = Vec::with_capacity(decisions.records.len());
            for record in &decisions.records {
                let mut rule = None;
                let mut features = Vec::with_capacity(record.considered.len());
                for candidate in &record.considered {
                    let matching = manifest_decisions
                        .iter()
                        .copied()
                        .filter(|decision| {
                            decision.input() == record.input
                                && decision.candidate() == candidate.candidate
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
                    let expected_verdict =
                        baseline_outcome_verdict(record.outcome, candidate.candidate);
                    if decision.verdict() != expected_verdict {
                        return Err(OptimizationRunError::ExternalDecisionManifestMismatch);
                    }
                    features.push(
                        ExternalCandidateFeatures::new(
                            *candidate,
                            decision.consumed_analyses(),
                            decision.consumed_facts().iter().copied(),
                        )
                        .map_err(OptimizationRunError::ExternalDecisionSchema)?,
                    );
                }
                points.push(
                    ExternalDecisionPoint::new(
                        record.input,
                        rule.ok_or(OptimizationRunError::ExternalDecisionManifestMismatch)?,
                        features,
                        record.outcome.into(),
                    )
                    .map_err(OptimizationRunError::ExternalDecisionSchema)?,
                );
            }
            Ok(points)
        }
    }
    pub(crate) mod replay {
        use std::collections::BTreeSet;

        use optimization_core::{
            BaselineDecisionOutcome, ExternalCandidateFeatures, ExternalDecisionAction,
            ExternalDecisionContext, ExternalDecisionLog, ExternalDecisionPoint,
        };
        use optimization_core::{OptimizationRuleIdentity, OptimizationUnitIdentity};

        use super::super::ExternalDecisionReplayError;
        use super::context;

        pub(crate) struct ExternalDecisionReplayCursor<'log> {
            points: &'log [ExternalDecisionPoint],
            next: usize,
        }

        impl<'log> ExternalDecisionReplayCursor<'log> {
            pub(crate) fn new(
                decisions: &'log ExternalDecisionLog,
                expected_context: ExternalDecisionContext,
            ) -> Result<Self, ExternalDecisionReplayError> {
                if let Some(axis) = context::mismatch(expected_context, decisions.context()) {
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

            pub(crate) fn choose(
                &mut self,
                input: OptimizationUnitIdentity,
                rule: OptimizationRuleIdentity,
                candidates: &[ExternalCandidateFeatures],
            ) -> Result<BaselineDecisionOutcome, ExternalDecisionReplayError> {
                let ordinal = self.next;
                let point = self.points.get(ordinal).ok_or(
                    ExternalDecisionReplayError::MissingDecision {
                        ordinal,
                        input,
                        rule,
                    },
                )?;
                let mut legal_candidates = candidates.to_vec();
                legal_candidates.sort_by_key(ExternalCandidateFeatures::candidate);
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
                    ExternalDecisionAction::Choose(candidate) => {
                        BaselineDecisionOutcome::Choose(candidate)
                    }
                    ExternalDecisionAction::Skip(reason) => BaselineDecisionOutcome::Skip(reason),
                })
            }

            pub(crate) fn require_exhausted(&self) -> Result<(), ExternalDecisionReplayError> {
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
    }

    use optimization_core::OptimizationRuleContract;
    use optimization_core::{ExternalCandidateFeatures, ExternalDecisionSchemaError};
    use optimization_unit::PsiCandidateDeclaration;

    pub(crate) use context::expected_context;
    pub(crate) use recording::external_points_from_manifest_decisions;
    pub(crate) use replay::ExternalDecisionReplayCursor;

    pub(crate) fn validated_candidate_features(
        candidate: &PsiCandidateDeclaration,
        contract: OptimizationRuleContract,
    ) -> Result<ExternalCandidateFeatures, ExternalDecisionSchemaError> {
        candidate_features::derive(candidate, contract)
    }
}

use optimization_core::TargetCostModelIdentity;

use crate::ranked_rewrites::LoopInvariantScalarMotionError;
use crate::validation::ValidatedOptimizerRankingCertificates;
use crate::validation::{
    ValidatedOptimizerCycleComponents, validate_transformed_psi_context,
    validate_verified_psi_context,
};
use crate::verified_unit::VerifiedPsiOptimizationUnit;
use crate::{AnalysisError, RuleProposalError, RuleRegistryError};
use crate::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantPlacementAnalysisError,
    CountedLoopAnalysisError, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops,
};
pub(crate) use execution::run_registries;
use optimization_core::{
    BaselineDecisionLog, BaselineDecisionLogDecodeError, BaselineDecisionOutcome,
    BaselineDecisionRecordError, ExternalDecisionLog, ExternalDecisionSchemaError,
    InvalidOptimizationManifestRecord, OptimizationCandidateIdentity, OptimizationCandidateVerdict,
    OptimizationIdentityBundle, OptimizationPassManifestRecord, OptimizationReasonCode,
    OptimizationRuleIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationWorkBudget,
};
use optimization_unit::{
    InvalidPsiTransformationLedger, PsiOptimizationUnit, PsiTransformationLedger,
};
pub use optimization_unit::{
    OptimizationRunUsage, PsiOptimizationCommit, PsiValidatedCandidateDeclaration,
};
use optimization_unit_semantics::OptimizationUnitValidationError;
use verified_optimization_input::VerifiedPsiOptimizationInput;

pub(crate) fn baseline_psi_cost_model_identity() -> TargetCostModelIdentity {
    TargetCostModelIdentity::from_canonical_bytes(b"omega.psi-baseline-structural-cost-model.v1")
}

pub(crate) fn baseline_outcome_verdict(
    outcome: BaselineDecisionOutcome,
    candidate: OptimizationCandidateIdentity,
) -> OptimizationCandidateVerdict {
    match outcome {
        BaselineDecisionOutcome::Choose(chosen) if chosen == candidate => {
            OptimizationCandidateVerdict::Applied
        }
        BaselineDecisionOutcome::Choose(_) => {
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::Superseded)
        }
        BaselineDecisionOutcome::Skip(reason) => OptimizationCandidateVerdict::Skipped(reason),
    }
}

#[derive(Debug)]
pub struct VerifiedPsiOptimizationSession {
    input: VerifiedPsiOptimizationInput,
    unit: PsiOptimizationUnit,
    cycle_components: ValidatedOptimizerCycleComponents,
}

impl VerifiedPsiOptimizationSession {
    pub fn new(
        verified: VerifiedPsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        let cycle_components = validate_verified_psi_context(&verified)?;
        let admitted_machines = cycle_components
            .components()
            .iter()
            .map(|component| component.id.machine)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let (input, unit) = verified.into_parts();
        let frontiers =
            optimization_unit_semantics::reconstruct_ownership_frontier_facts_with_admitted_cycle_machines(
                &unit,
                &admitted_machines,
            )?;
        let unit = optimization_unit::attach_ownership_frontier_facts(unit, frontiers)
            .map_err(|_| OptimizationUnitValidationError::OwnershipFrontierFactIndexMismatch)?;
        optimization_unit_semantics::validate_psi_optimization_unit_with_admitted_cycle_machines(
            &unit,
            &admitted_machines,
        )?;
        Ok(Self {
            input,
            unit,
            cycle_components,
        })
    }

    pub(crate) fn from_transformed(
        input: VerifiedPsiOptimizationInput,
        unit: PsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        let cycle_components = validate_transformed_psi_context(&input, &unit)?;
        Ok(Self {
            input,
            unit,
            cycle_components,
        })
    }

    pub const fn input(&self) -> &VerifiedPsiOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(self) -> (VerifiedPsiOptimizationInput, PsiOptimizationUnit) {
        (self.input, self.unit)
    }
}

impl VerifiedPsiOptimizationSession {
    pub const fn cycle_components(&self) -> &ValidatedOptimizerCycleComponents {
        &self.cycle_components
    }

    pub const fn ranking_certificates(&self) -> &ValidatedOptimizerRankingCertificates {
        self.cycle_components.ranking_certificates()
    }

    pub(crate) fn counted_loop_analysis(
        &self,
    ) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
        analyze_counted_loops(&self.unit, &self.cycle_components)
    }

    pub(crate) fn countdown_invariant_constant_analysis(
        &self,
    ) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError>
    {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantAnalysisError::CountedLoop)?;
        analyze_countdown_invariant_constants(&self.unit, &self.cycle_components, &counted)
    }

    fn countdown_invariant_inputs(
        &self,
    ) -> Result<
        (
            ValidatedCountedLoopAnalysis,
            ValidatedCountdownInvariantConstantAnalysis,
        ),
        CountdownInvariantConstantPlacementAnalysisError,
    > {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantPlacementAnalysisError::CountedLoop)?;
        let invariants =
            analyze_countdown_invariant_constants(&self.unit, &self.cycle_components, &counted)
                .map_err(CountdownInvariantConstantPlacementAnalysisError::InvariantConstant)?;
        Ok((counted, invariants))
    }

    pub(crate) fn countdown_invariant_constant_placement_analysis(
        &self,
    ) -> Result<
        ValidatedCountdownInvariantConstantPlacementAnalysis,
        CountdownInvariantConstantPlacementAnalysisError,
    > {
        let (counted, invariants) = self.countdown_invariant_inputs()?;
        analyze_countdown_invariant_constant_placement(
            &self.unit,
            &self.cycle_components,
            &counted,
            &invariants,
        )
    }
}

#[derive(Debug)]
pub struct OptimizationRun {
    pub selections: OptimizationSelections,
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub session: VerifiedPsiOptimizationSession,
    pub commits: Vec<PsiOptimizationCommit>,
    pub validated_candidates: Vec<PsiValidatedCandidateDeclaration>,
    pub usage: OptimizationRunUsage,
    pub decisions: BaselineDecisionLog,
    pub external_decisions: ExternalDecisionLog,
    pub pass_manifests: Vec<OptimizationPassManifestRecord>,
    pub transformation_ledger: PsiTransformationLedger,
    pub identity_bundle: OptimizationIdentityBundle,
}

impl OptimizationRun {
    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn psi_selections(&self) -> &OptimizationSelections {
        &self.psi_selections
    }

    pub const fn session(&self) -> &VerifiedPsiOptimizationSession {
        &self.session
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }

    pub const fn usage(&self) -> OptimizationRunUsage {
        self.usage
    }

    pub const fn decisions(&self) -> &BaselineDecisionLog {
        &self.decisions
    }

    pub fn pass_manifests(&self) -> &[OptimizationPassManifestRecord] {
        &self.pass_manifests
    }

    pub const fn transformation_ledger(&self) -> &PsiTransformationLedger {
        &self.transformation_ledger
    }

    pub const fn identity_bundle(&self) -> OptimizationIdentityBundle {
        self.identity_bundle
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationRunError {
    InitialValidation(OptimizationUnitValidationError),
    Analysis(AnalysisError),
    Proposal {
        rule: OptimizationRuleIdentity,
        error: RuleProposalError,
    },
    CandidateValidation(OptimizationUnitValidationError),
    LoopInvariantScalarMotion(LoopInvariantScalarMotionError),
    WorkBudgetExhausted(&'static str),
    NonDecreasingConvergenceMeasure {
        previous: u64,
        current: u64,
    },
    OscillatingRevision {
        identity: OptimizationUnitIdentity,
        first_seen_iteration: u64,
        repeated_at_iteration: u64,
    },
    RegistryCoverageMismatch,
    DuplicateCandidate(OptimizationCandidateIdentity),
    CandidateContractMismatch {
        candidate: OptimizationCandidateIdentity,
        axis: CandidateContractAxis,
    },
    PolicySelectionMissing(OptimizationCandidateIdentity),
    InvalidManifest(InvalidOptimizationManifestRecord),
    InvalidTransformationLedger(InvalidPsiTransformationLedger),
    DecisionLogReplay(BaselineDecisionLogDecodeError),
    ExternalDecisionSchema(ExternalDecisionSchemaError),
    ExternalDecisionManifestMismatch,
    ExternalDecisionReplay(ExternalDecisionReplayError),
    WorkUsageOverflow,
    MissingPassManifest,
    DuplicatePipelineRule,
    RegistryConstruction(RuleRegistryError),
    SelectionRegistryMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateContractAxis {
    Input,
    Rule,
    RequiredAnalyses,
    InvalidatedAnalyses,
    SafetyClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalDecisionReplayError {
    Schema(ExternalDecisionSchemaError),
    ContextMismatch(ExternalDecisionContextAxis),
    DuplicateDecision {
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
    },
    MissingDecision {
        ordinal: usize,
        input: OptimizationUnitIdentity,
        rule: OptimizationRuleIdentity,
    },
    IllegalDecision {
        ordinal: usize,
        expected_input: OptimizationUnitIdentity,
        expected_rule: OptimizationRuleIdentity,
    },
    InvalidRecordedOutcome(BaselineDecisionRecordError),
    LeftoverDecisions {
        first_unused: usize,
        remaining: usize,
    },
    ReconstructedLogMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalDecisionContextAxis {
    Schema,
    Source,
    Selections,
    PhaseSelections,
    Target,
    RuleSet,
    CostModel,
}

failed_display!(
    ExternalDecisionReplayError,
    "external Psi decision replay failed"
);
failed_display!(OptimizationRunError, "Psi optimization run failed");
