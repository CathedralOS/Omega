use std::collections::{BTreeMap, BTreeSet};

use omega_optimization_core::{
    InvalidOptimizationManifestRecord, OptimizationCandidateIdentity, OptimizationCandidateVerdict,
    OptimizationDecisionRecord, OptimizationExecutionPhase, OptimizationIdentityBundle,
    OptimizationPassManifestRecord, OptimizationReasonCode, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OptimizationSelections, OptimizationUnitIdentity,
    OptimizationValidatorIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    TargetCostModelIdentity,
};
use omega_optimization_policy::{
    BaselineDecisionLog, BaselineDecisionLogDecodeError, BaselineDecisionOutcome, BaselinePolicy,
    ValidatedCandidateSummary,
};
use omega_optimization_unit::{
    InvalidPsiTransformationLedger, ProvenanceRewrite, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiTransformationLedger, PsiTransformationRecord,
};
use omega_optimization_validation::{
    OptimizationUnitValidationError, ValidatedPsiRewrite, validate_psi_rewrite_candidate,
    validate_verified_psi_optimization_unit,
};
use omega_terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationUnit, VerifiedTerminalOptimizationInput,
};

use crate::{
    AnalysisManager, AnalysisManagerError, OrderedRuleRegistry, RuleAnalysisView,
    RuleProposalError, RuleRegistryError, built_in_psi_registries, built_in_psi_registry,
};

pub fn baseline_psi_cost_model_identity() -> TargetCostModelIdentity {
    TargetCostModelIdentity::from_canonical_bytes(b"omega.psi-baseline-structural-cost-model.v1")
}

#[derive(Debug)]
pub struct VerifiedPsiOptimizationSession {
    input: VerifiedTerminalOptimizationInput,
    unit: PsiOptimizationUnit,
}

impl VerifiedPsiOptimizationSession {
    pub fn new(
        verified: VerifiedPsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        validate_verified_psi_optimization_unit(&verified)?;
        let (input, unit) = verified.into_parts();
        Ok(Self { input, unit })
    }

    pub const fn input(&self) -> &VerifiedTerminalOptimizationInput {
        &self.input
    }

    pub const fn unit(&self) -> &PsiOptimizationUnit {
        &self.unit
    }

    pub fn into_parts(self) -> (VerifiedTerminalOptimizationInput, PsiOptimizationUnit) {
        (self.input, self.unit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiOptimizationCommit {
    pub rule: OptimizationRuleIdentity,
    pub candidate: OptimizationCandidateIdentity,
    pub validator: OptimizationValidatorIdentity,
    pub input: OptimizationUnitIdentity,
    pub output: OptimizationUnitIdentity,
    pub predicted_cost_delta: i64,
    pub pruned_machines: Vec<omega_optimization_unit::PrunedMachineCustody>,
    pub provenance: Vec<ProvenanceRewrite>,
    pub declaration: PsiRewriteCandidate,
}

impl PsiOptimizationCommit {
    pub const fn declaration(&self) -> &PsiRewriteCandidate {
        &self.declaration
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizationRunUsage {
    pub rule_evaluations: u64,
    pub candidates: u64,
    pub validation_steps: u64,
    pub commits: u64,
    pub iterations: u64,
}

#[derive(Debug)]
pub struct OptimizationRun {
    /// Complete source-visible suite requested by the root build.
    pub selections: OptimizationSelections,
    /// Exact subset executed by this Psi-phase run.
    pub psi_selections: OptimizationSelections,
    pub budget_per_pass: OptimizationWorkBudget,
    pub session: VerifiedPsiOptimizationSession,
    pub commits: Vec<PsiOptimizationCommit>,
    pub usage: OptimizationRunUsage,
    pub decisions: BaselineDecisionLog,
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

    pub fn commits(&self) -> &[PsiOptimizationCommit] {
        &self.commits
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
    Analysis(AnalysisManagerError),
    Proposal {
        rule: OptimizationRuleIdentity,
        error: RuleProposalError,
    },
    CandidateValidation(OptimizationUnitValidationError),
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
    PolicySelectionMissing(OptimizationCandidateIdentity),
    InvalidManifest(InvalidOptimizationManifestRecord),
    InvalidTransformationLedger(InvalidPsiTransformationLedger),
    DecisionLogReplay(BaselineDecisionLogDecodeError),
    WorkUsageOverflow,
    MissingPassManifest,
    DuplicatePipelineRule,
    RegistryConstruction(RuleRegistryError),
    SelectionRegistryMismatch,
}

impl std::fmt::Display for OptimizationRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Psi optimization run failed: {self:?}")
    }
}

impl std::error::Error for OptimizationRunError {}

pub fn run_psi_registry(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let expected =
        built_in_psi_registry(selections).map_err(OptimizationRunError::RegistryConstruction)?;
    if expected.identity() != registry.identity()
        || expected.contracts().collect::<Vec<_>>() != registry.contracts().collect::<Vec<_>>()
    {
        return Err(OptimizationRunError::SelectionRegistryMismatch);
    }
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    if registry.is_empty() {
        run_registries(session, selections, &[], budget)
    } else {
        run_registries(session, selections, std::slice::from_ref(registry), budget)
    }
}

/// Execute every implemented named optimization as its own canonical pass
/// group and publish one chained run over the exact selected suite.
pub fn run_psi_pipeline(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let registries =
        built_in_psi_registries(selections).map_err(OptimizationRunError::RegistryConstruction)?;
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    run_registries(session, selections, &registries, budget_per_pass)
}

fn run_registries(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    let psi_selections = selections.for_phase(OptimizationExecutionPhase::Psi);
    let initial_identity = session.unit.identity;
    let terminal_psi = session.unit.terminal_psi;
    let fuel_schedule = session.unit.fuel_schedule;
    let ordered_rules = registries
        .iter()
        .flat_map(OrderedRuleRegistry::contracts)
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
    let ordered_rule_set = OptimizationRuleSetIdentity::from_ordered_rules(&ordered_rules)
        .map_err(|_| OptimizationRunError::DuplicatePipelineRule)?;
    let mut unit = session.unit;
    let mut commits = Vec::new();
    let mut usage = OptimizationRunUsage::default();
    let mut pass_logs = Vec::with_capacity(registries.len());
    let mut pass_manifests = Vec::with_capacity(registries.len());
    for registry in registries {
        let (output, pass_commits, pass_usage, pass_decisions, pass_manifest, _pass_ledger) =
            run_unit(unit, registry, budget_per_pass)?;
        unit = output;
        commits.extend(pass_commits);
        usage = add_usage(usage, pass_usage)?;
        pass_logs.push(pass_decisions);
        pass_manifests.push(pass_manifest.ok_or(OptimizationRunError::MissingPassManifest)?);
    }
    let decisions = BaselineDecisionLog::concatenate(&pass_logs)
        .map_err(OptimizationRunError::DecisionLogReplay)?;
    let transformation_ledger = PsiTransformationLedger::new(
        terminal_psi,
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
        pass_manifests,
        transformation_ledger,
        identity_bundle,
    })
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

type OptimizationRunOutput = (
    PsiOptimizationUnit,
    Vec<PsiOptimizationCommit>,
    OptimizationRunUsage,
    BaselineDecisionLog,
    Option<OptimizationPassManifestRecord>,
    PsiTransformationLedger,
);

fn run_unit(
    mut unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    let mut analyses = AnalysisManager::new(&unit);
    let initial_identity = unit.identity;
    let terminal_psi = unit.terminal_psi;
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
            let outcome = policy.choose(
                unit.identity,
                validated
                    .iter()
                    .map(|(candidate, _)| ValidatedCandidateSummary {
                        candidate: candidate.identity(),
                        predicted_cost_delta: candidate.predicted_cost_delta(),
                    }),
            );
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
                terminal_psi,
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

fn register_revision(
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

fn build_pass_manifest(
    registry: &OrderedRuleRegistry,
    input: OptimizationUnitIdentity,
    output: OptimizationUnitIdentity,
    commits: &[PsiOptimizationCommit],
    decisions: &[OptimizationDecisionRecord],
    usage: OptimizationRunUsage,
) -> Result<Option<OptimizationPassManifestRecord>, OptimizationRunError> {
    let Some(pass) = registry.pass() else {
        return Ok(None);
    };
    let contracts = registry.contracts().collect::<Vec<_>>();
    let ordered_rules = contracts
        .iter()
        .map(|contract| contract.identity())
        .collect::<Vec<_>>();
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
        registry.identity(),
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

fn charge(counter: &mut u64, limit: u64, axis: &'static str) -> Result<(), OptimizationRunError> {
    if *counter == limit {
        return Err(OptimizationRunError::WorkBudgetExhausted(axis));
    }
    *counter += 1;
    Ok(())
}

fn integer_evaluation_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerAdd {
                    ..
                } | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerMultiply { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerAdd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerMultiply { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerAdd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerMultiply { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerDivide { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerRemainder { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerDivide { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerRemainder { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerDivide { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerRemainder { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerShiftRight { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerShiftRight { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerExactCast { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerWiden { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseNot { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseAnd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseOr { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseXor { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanNot { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanEqual { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerEqual { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerLessThan { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerLessOrEqual { .. }
            )
        })
        .count()
        .try_into()
        .expect("operation count fits u64")
}

fn block_parameter_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .map(|block| u64::try_from(block.parameters.len()).expect("parameter count fits u64"))
        .sum()
}

fn dead_total_scalar_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerConstant { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanConstant { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanNot { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::BooleanEqual { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerEqual { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerLessThan { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerLessOrEqual { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseNot { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseAnd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseOr { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerBitwiseXor { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerWiden { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerShiftLeft { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerShiftRight { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerAdd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerMultiply { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerAdd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerMultiply { .. }
            )
        })
        .count()
        .try_into()
        .expect("dead-total scalar operation count fits u64")
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

fn convergence_measure(unit: &PsiOptimizationUnit, registry: &OrderedRuleRegistry) -> u64 {
    let copy_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.copy-propagation.v1",
    );
    let cfg_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.control-flow-cleanup.v10",
    );
    let dead_scalar_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.dead-pure-scalar-elimination.v2",
    );
    if registry.pass() == Some(cfg_pass) {
        control_flow_structure_count(unit)
    } else if registry.pass() == Some(copy_pass) {
        block_parameter_count(unit)
    } else if registry.pass() == Some(dead_scalar_pass) {
        dead_total_scalar_operation_count(unit)
    } else {
        integer_evaluation_operation_count(unit)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use omega_optimization_core::{
        Optimization, OptimizationFactReference, OptimizationSelections,
    };
    use omega_optimization_unit::PsiRewritePatch;
    use omega_terminal_abstract_operations::TerminalAbstractOperation;

    use super::*;
    use crate::{
        AnalysisProduct, ExactIntegerAddConstantsRule, PsiOptimizationRule,
        built_in_psi_registries, built_in_psi_registry,
        rules::tests::{
            boolean_unit, constant_conditional_same_target_unit, dead_wrapping_add_unit,
            dependent_exact_chain_unit, exact_add_unit, linear_empty_block_unit,
            propagated_block_parameter_unit, randomized_sccp_registries,
            redundant_block_parameter_unit, wrapping_add_unit,
        },
    };

    #[derive(Debug)]
    struct NonProfitableExactRule;

    impl PsiOptimizationRule for NonProfitableExactRule {
        fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
            ExactIntegerAddConstantsRule::contract()
        }

        fn propose(
            &self,
            unit: &PsiOptimizationUnit,
            analyses: RuleAnalysisView<'_>,
        ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
            ExactIntegerAddConstantsRule
                .propose(unit, analyses)?
                .into_iter()
                .map(|candidate| {
                    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(patch) =
                        candidate.patch()
                    else {
                        return Err(RuleProposalError::InvalidCandidate(
                            omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch,
                        ));
                    };
                    omega_optimization_unit::PsiRewriteCandidate::new_integer_evaluation(
                        candidate.input(),
                        Self.contract(),
                        candidate.affected_blocks().to_vec(),
                        candidate.substitutions().to_vec(),
                        candidate.provenance().to_vec(),
                        candidate.scalar_evaluation_witness().unwrap(),
                        0,
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)
                })
                .collect()
        }
    }

    #[derive(Debug)]
    struct DuplicateExactRule;

    impl PsiOptimizationRule for DuplicateExactRule {
        fn contract(&self) -> omega_optimization_core::OptimizationRuleContract {
            ExactIntegerAddConstantsRule::contract()
        }

        fn propose(
            &self,
            unit: &PsiOptimizationUnit,
            analyses: RuleAnalysisView<'_>,
        ) -> Result<Vec<omega_optimization_unit::PsiRewriteCandidate>, RuleProposalError> {
            let mut candidates = ExactIntegerAddConstantsRule.propose(unit, analyses)?;
            candidates.push(candidates[0].clone());
            Ok(candidates)
        }
    }

    fn verified_empty_unit() -> VerifiedPsiOptimizationUnit {
        use psi_core::{BlockId, ContractId, EdgeId, MachineId};
        use psi_terminal::{
            Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule,
            Terminator, VocabularyMarker,
        };

        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: MachineId::new(401).unwrap(),
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: MachineId::new(401).unwrap(),
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: BlockId::new(402).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(402).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(403).unwrap(),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(404).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let semantic = psi_terminal_codec::encode_module(&module).unwrap();
        let proof =
            psi_terminal_codec::encode_proof_bundle(&psi_terminal_verifier::ProofBundle::default())
                .unwrap();
        let input =
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .unwrap();
        omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap()
    }

    fn verified_exact_add_unit() -> VerifiedPsiOptimizationUnit {
        use psi_core::{
            BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
            ObligationId, OperationId, ScalarType, ValueId,
        };
        use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
        use psi_terminal::{
            Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
            TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
        };
        use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

        let machine = MachineId::new(411).unwrap();
        let block = BlockId::new(412).unwrap();
        let left = ValueId::new(413).unwrap();
        let right = ValueId::new(414).unwrap();
        let computed = ValueId::new(415).unwrap();
        let result = ValueId::new(422).unwrap();
        let obligation = ObligationId::new(419).unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let module = TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            proof_output_calls: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            closed_conformance_applications: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    id: block,
                    parameters: Vec::new(),
                    operations: vec![
                        Operation {
                            id: OperationId::new(416).unwrap(),
                            result: OperationResult::Scalar(declaration(left)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                        Operation {
                            id: OperationId::new(417).unwrap(),
                            result: OperationResult::Scalar(declaration(right)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(8),
                            },
                        },
                        Operation {
                            id: OperationId::new(418).unwrap(),
                            result: OperationResult::Scalar(declaration(computed)),
                            kind: OperationKind::ExactIntegerAdd {
                                left,
                                right,
                                obligation,
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(420).unwrap(),
                        value: computed,
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(421).unwrap(),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            }],
        };
        let semantic = psi_terminal_codec::encode_module(&module).unwrap();
        let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
        let input =
            omega_terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
                &semantic,
                &proof,
                &psi_proof_admission::AdmissionProfile::default(),
            )
            .unwrap();
        omega_terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
            input,
            psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
        )
        .unwrap()
    }

    fn budget(iterations: u64) -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(64, 64, 64, 64, iterations).unwrap()
    }

    fn run_test_pipeline(
        mut unit: PsiOptimizationUnit,
        registries: &[OrderedRuleRegistry],
    ) -> (
        PsiOptimizationUnit,
        Vec<OptimizationPassManifestRecord>,
        PsiTransformationLedger,
    ) {
        let input = unit.identity;
        let terminal_psi = unit.terminal_psi;
        let fuel_schedule = unit.fuel_schedule;
        let mut manifests = Vec::with_capacity(registries.len());
        let mut records = Vec::new();
        for registry in registries {
            let (output, _, _, _, manifest, ledger) = run_unit(unit, registry, budget(8)).unwrap();
            manifests.push(manifest.expect("a selected pass emits a manifest row"));
            records.extend_from_slice(ledger.records());
            unit = output;
        }
        let ledger = PsiTransformationLedger::new(
            terminal_psi,
            fuel_schedule,
            input,
            unit.identity,
            records,
        )
        .unwrap();
        (unit, manifests, ledger)
    }

    #[test]
    fn fixed_point_dispatch_validates_then_commits_with_stable_usage() {
        let unit = exact_add_unit();
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, decisions, pass_manifest, ledger) =
            run_unit(unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].input, unit.identity);
        assert_eq!(commits[0].output, output.identity);
        assert_eq!(usage.commits, 1);
        assert_eq!(usage.validation_steps, 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.rule_evaluations, 31);
        assert_eq!(decisions.records.len(), 1);
        assert_eq!(
            decisions.records[0].outcome,
            BaselineDecisionOutcome::Choose(commits[0].candidate)
        );
        let pass_manifest = pass_manifest.expect("selected pass emits a manifest row");
        assert_eq!(pass_manifest.ordered_rules().len(), 30);
        assert_eq!(pass_manifest.input(), unit.identity);
        assert_eq!(pass_manifest.output(), output.identity);
        assert_eq!(pass_manifest.decisions().len(), 1);
        assert_eq!(pass_manifest.decisions()[0].input(), unit.identity);
        assert_eq!(pass_manifest.decisions()[0].consumed_facts().len(), 3);
        assert_eq!(
            pass_manifest.decisions()[0].verdict(),
            OptimizationCandidateVerdict::Applied
        );
        assert_eq!(
            OptimizationPassManifestRecord::decode(&pass_manifest.encode()),
            Ok(pass_manifest)
        );
        assert_eq!(ledger.input(), unit.identity);
        assert_eq!(ledger.output(), output.identity);
        assert_eq!(ledger.records().len(), 1);
        assert_eq!(ledger.records()[0].provenance, commits[0].provenance);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant { .. }
        ));
    }

    #[test]
    fn named_control_flow_cleanup_reaches_edge_count_fixed_point() {
        let unit = constant_conditional_same_target_unit(true);
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(usage.commits, 2);
        assert_eq!(usage.iterations, 3);
        assert_eq!(output.functions[0].blocks.len(), 1);
        assert_eq!(ledger.records().len(), 2);
        assert_eq!(ledger.records()[0].provenance.len(), 2);
        assert!(matches!(
            ledger.records()[0].provenance[0].disposition,
            omega_optimization_unit::ProvenanceDisposition::RealizedAt(_)
        ));
        assert!(matches!(
            ledger.records()[0].provenance[1].disposition,
            omega_optimization_unit::ProvenanceDisposition::ProvenUnreachableAt(_)
        ));
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 6);
        assert_eq!(manifest.decisions().len(), 2);
        assert_eq!(manifest.decisions()[0].consumed_facts().len(), 1);

        let (second, second_commits, _, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_control_flow_cleanup_atomically_prunes_and_accounts_for_a_dead_arm() {
        let unit = propagated_block_parameter_unit(true);
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit.clone(), &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 3);
        assert_eq!(usage.commits, 3);
        assert_eq!(usage.iterations, 4);
        assert_eq!(unit.functions[0].blocks.len(), 4);
        assert_eq!(output.functions[0].blocks.len(), 1);
        assert_eq!(ledger.records().len(), 3);
        assert_eq!(ledger.records()[0].provenance.len(), 6);
        assert_eq!(
            ledger.records()[0]
                .provenance
                .iter()
                .filter(|row| row.disposition.is_realized())
                .count(),
            3
        );
        assert_eq!(
            ledger.records()[0]
                .provenance
                .iter()
                .filter(|row| !row.disposition.is_realized())
                .count(),
            3
        );
        assert_eq!(output.functions[0].facts.len(), 2);
        assert_eq!(output.functions[0].blocks[0].nodes[0].effect.input, 0);
        assert_eq!(output.functions[0].blocks[0].nodes[3].effect.output, 4);
        assert_eq!(manifest.unwrap().decisions().len(), 4);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_control_flow_cleanup_threads_a_linear_empty_block_to_fixed_point() {
        let unit = linear_empty_block_unit();
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit.clone(), &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(usage.commits, 2);
        assert_eq!(usage.iterations, 3);
        assert_eq!(usage.rule_evaluations, 12);
        assert_eq!(output.functions[0].blocks.len(), 1);
        assert_eq!(ledger.records().len(), 2);
        assert_eq!(ledger.records()[0].provenance.len(), 3);
        assert!(
            ledger.records()[0]
                .provenance
                .iter()
                .all(|row| row.disposition.is_realized())
        );
        assert_eq!(manifest.unwrap().ordered_rules().len(), 6);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn ordered_multi_rule_group_reaches_a_dependent_exact_fixed_point() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, pass_manifest, ledger) =
            run_unit(dependent_exact_chain_unit(), &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 2);
        assert_eq!(usage.iterations, 3);
        assert_eq!(usage.rule_evaluations, 34);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(15),
                ..
            }
        ));
        assert!(matches!(
            output.functions[0].blocks[0].nodes[3].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(120),
                ..
            }
        ));
        let manifest = pass_manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 30);
        assert_eq!(manifest.decisions().len(), 2);
        assert_eq!(ledger.records().len(), 2);
    }

    #[test]
    fn named_dead_scalar_suite_reaches_a_custody_preserving_fixed_point() {
        let selections =
            OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let unit = dead_wrapping_add_unit();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 3);
        assert_eq!(usage.iterations, 4);
        assert_eq!(output.functions[0].blocks[0].nodes.len(), 1);
        assert_eq!(output.functions[0].blocks[0].nodes[0].provenance.len(), 4);
        assert_eq!(ledger.records().len(), 3);
        assert!(
            ledger
                .records()
                .iter()
                .flat_map(|record| &record.provenance)
                .all(|row| matches!(
                    row.disposition,
                    omega_optimization_unit::ProvenanceDisposition::RealizedAt(_)
                ))
        );
        assert_eq!(manifest.unwrap().ordered_rules().len(), 2);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn shuffled_builtin_registration_constructs_identical_sccp_runs() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let expected_registry = built_in_psi_registry(&selections).unwrap();
        let expected =
            run_unit(dependent_exact_chain_unit(), &expected_registry, budget(8)).unwrap();

        for registry in randomized_sccp_registries() {
            let actual = run_unit(dependent_exact_chain_unit(), &registry, budget(8)).unwrap();
            assert_eq!(actual.0, expected.0);
            assert_eq!(actual.1, expected.1);
            assert_eq!(actual.2, expected.2);
            assert_eq!(actual.3, expected.3);
            assert_eq!(actual.4, expected.4);
            assert_eq!(actual.5, expected.5);
        }
    }

    #[test]
    fn full_sccp_cfg_copy_dead_scalar_second_sweep_is_a_composed_ledger_fixed_point() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
            Optimization::DeadPureScalarElimination,
        ])
        .unwrap();
        let registries = built_in_psi_registries(&selections).unwrap();

        for initial in [
            dependent_exact_chain_unit(),
            redundant_block_parameter_unit(true),
            dead_wrapping_add_unit(),
        ] {
            let (first_output, first_manifests, first_ledger) =
                run_test_pipeline(initial, &registries);
            assert_eq!(first_manifests.len(), 4);
            assert_eq!(first_manifests[0].input(), first_ledger.input());
            assert_eq!(first_manifests[0].output(), first_manifests[1].input());
            assert_eq!(first_manifests[1].output(), first_manifests[2].input());
            assert_eq!(first_manifests[2].output(), first_manifests[3].input());
            assert_eq!(first_manifests[3].output(), first_ledger.output());
            assert!(!first_ledger.records().is_empty());

            let (second_output, second_manifests, second_delta) =
                run_test_pipeline(first_output.clone(), &registries);
            assert_eq!(second_output, first_output);
            assert_eq!(second_manifests.len(), 4);
            assert!(second_delta.records().is_empty());
            assert_eq!(second_delta.input(), second_delta.output());
            assert!(second_manifests.iter().all(|manifest| {
                manifest.input() == manifest.output()
                    && manifest
                        .decisions()
                        .iter()
                        .all(|decision| decision.verdict() != OptimizationCandidateVerdict::Applied)
            }));

            let mut composed_records = first_ledger.records().to_vec();
            composed_records.extend_from_slice(second_delta.records());
            let composed = PsiTransformationLedger::new(
                first_ledger.terminal_psi(),
                first_ledger.fuel_schedule(),
                first_ledger.input(),
                second_delta.output(),
                composed_records,
            )
            .unwrap();
            assert_eq!(composed, first_ledger);
        }
    }

    #[test]
    fn manifest_retains_propagated_block_parameter_fact_identity() {
        let unit = propagated_block_parameter_unit(true);
        let AnalysisProduct::ScalarConstants(constants) = crate::compute_analysis(
            &unit,
            omega_optimization_core::AnalysisKind::ScalarConstants,
        )
        .unwrap() else {
            unreachable!()
        };
        let derived = constants
            .facts
            .iter()
            .find(|fact| !fact.support.edges.is_empty())
            .and_then(|fact| fact.identity)
            .expect("fixture has one proof-bearing propagated parameter fact");
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (_, commits, _, _, manifest, _) = run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(
            manifest.unwrap().decisions()[0].consumed_facts(),
            &[OptimizationFactReference::ScalarConstant(derived)]
        );
    }

    #[test]
    fn named_copy_propagation_reaches_its_block_parameter_fixed_point() {
        let unit = redundant_block_parameter_unit(true);
        let selections = OptimizationSelections::new([Optimization::CopyPropagation]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit.clone(), &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.commits, 1);
        assert!(output.functions[0].blocks[1].parameters.is_empty());
        assert_eq!(ledger.records().len(), 1);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.decisions().len(), 1);
        assert!(manifest.decisions()[0].consumed_facts().is_empty());
        assert_eq!(manifest.decisions()[0].input(), unit.identity);
        assert_eq!(manifest.output(), output.identity);
    }

    #[test]
    fn pass_convergence_measure_includes_wrapping_and_saturating_rules() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, _, _) =
            run_unit(wrapping_add_unit(), &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(4),
                ..
            }
        ));
    }

    #[test]
    fn pass_dispatches_typed_boolean_validation_to_fixed_point() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, _, _) =
            run_unit(boolean_unit(true), &registry, budget(8)).unwrap();

        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert!(matches!(
            output.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::BooleanConstant { value: false, .. }
        ));
    }

    #[test]
    fn exhausted_iteration_budget_fails_deterministically_without_output() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let first = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
        let second = run_unit(exact_add_unit(), &registry, budget(1)).unwrap_err();
        assert_eq!(first, second);
        assert_eq!(
            first,
            OptimizationRunError::WorkBudgetExhausted("iterations")
        );
    }

    #[test]
    fn synthetic_a_to_b_to_a_revision_cycle_fails_before_repeated_commit() {
        let a = OptimizationUnitIdentity::from_canonical_bytes(b"synthetic-state-a");
        let b = OptimizationUnitIdentity::from_canonical_bytes(b"synthetic-state-b");

        let run = || {
            let mut seen = BTreeMap::from([(a, 0)]);
            let mut committed = Vec::new();
            register_revision(&mut seen, b, 1)?;
            committed.push(b);
            let error = register_revision(&mut seen, a, 2).unwrap_err();
            Ok::<_, OptimizationRunError>((committed, error, seen))
        };

        let first = run().unwrap();
        let second = run().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.0, vec![b]);
        assert_eq!(first.2, BTreeMap::from([(a, 0), (b, 1)]));
        assert_eq!(
            first.1,
            OptimizationRunError::OscillatingRevision {
                identity: a,
                first_seen_iteration: 0,
                repeated_at_iteration: 2,
            }
        );
    }

    #[test]
    fn nonprofitable_validated_candidate_is_recorded_as_a_skip() {
        let registry = OrderedRuleRegistry::new([
            Arc::new(NonProfitableExactRule) as Arc<dyn PsiOptimizationRule>
        ])
        .unwrap();
        let (unit, commits, _, decisions, pass_manifest, ledger) =
            run_unit(exact_add_unit(), &registry, budget(2)).unwrap();

        assert!(commits.is_empty());
        assert_eq!(decisions.records.len(), 1);
        assert!(matches!(
            unit.functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::ExactIntegerAdd { .. }
        ));
        let manifest = pass_manifest.unwrap();
        assert_eq!(manifest.decisions().len(), 1);
        assert_eq!(
            manifest.decisions()[0].verdict(),
            OptimizationCandidateVerdict::Skipped(OptimizationReasonCode::NotProfitable)
        );
        assert!(manifest.decisions()[0].validator().is_some());
        assert_eq!(manifest.decisions()[0].consumed_facts().len(), 3);
        assert!(ledger.records().is_empty());
        assert_eq!(ledger.input(), ledger.output());
    }

    #[test]
    fn duplicate_candidate_identity_fails_closed_without_a_manifest() {
        let registry =
            OrderedRuleRegistry::new(
                [Arc::new(DuplicateExactRule) as Arc<dyn PsiOptimizationRule>],
            )
            .unwrap();
        assert!(matches!(
            run_unit(exact_add_unit(), &registry, budget(2)),
            Err(OptimizationRunError::DuplicateCandidate(_))
        ));
    }

    #[test]
    fn public_run_requires_and_retains_verified_optimizer_context() {
        let selections = OptimizationSelections::default();
        let registry = OrderedRuleRegistry::new(Vec::new()).unwrap();
        let run =
            run_psi_registry(verified_empty_unit(), &selections, &registry, budget(2)).unwrap();
        assert!(run.commits.is_empty());
        assert!(run.pass_manifests.is_empty());
        assert!(run.transformation_ledger.records().is_empty());
        assert_eq!(run.identity_bundle.selections(), selections.identity());
        assert_eq!(
            run.identity_bundle.transformation_ledger(),
            run.transformation_ledger.identity()
        );
        assert_eq!(run.usage.iterations, 0);
        assert_eq!(
            run.session.unit().terminal_psi,
            run.session.input().plan().terminal_psi
        );
    }

    #[test]
    fn lower_only_suite_retains_the_request_but_executes_no_psi_pass() {
        let selections =
            OptimizationSelections::new([Optimization::SelectedIncomingU12ExactAddImmediate])
                .unwrap();
        let run = run_psi_pipeline(verified_empty_unit(), &selections, budget(2)).unwrap();

        assert_eq!(run.selections(), &selections);
        assert!(run.psi_selections().is_empty());
        assert_eq!(run.identity_bundle.selections(), selections.identity());
        assert_eq!(
            run.identity_bundle.rule_set(),
            OptimizationRuleSetIdentity::from_ordered_rules(&[]).unwrap()
        );
        assert!(run.commits.is_empty());
        assert!(run.pass_manifests.is_empty());
        assert!(run.decisions.records.is_empty());
        assert!(run.transformation_ledger.records().is_empty());
        assert_eq!(run.usage, OptimizationRunUsage::default());
        assert_eq!(
            run.transformation_ledger.input(),
            run.transformation_ledger.output()
        );
    }

    #[test]
    fn mixed_suite_executes_only_its_psi_projection() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::SelectedIncomingU12ExactAddImmediate,
        ])
        .unwrap();
        let psi_selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let run = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
        let registry = built_in_psi_registry(&psi_selections).unwrap();

        assert_eq!(run.selections(), &selections);
        assert_eq!(run.psi_selections(), &psi_selections);
        assert_eq!(run.identity_bundle.selections(), selections.identity());
        assert_eq!(run.identity_bundle.rule_set(), registry.identity());
        assert_eq!(run.pass_manifests.len(), 1);
        assert_eq!(run.commits.len(), 1);
    }

    #[test]
    fn public_run_folds_proof_admitted_exact_arithmetic_and_retains_its_context() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let run =
            run_psi_registry(verified_exact_add_unit(), &selections, &registry, budget(8)).unwrap();

        assert_eq!(run.commits.len(), 1);
        assert_eq!(run.transformation_ledger.records().len(), 1);
        assert_eq!(run.pass_manifests[0].decisions().len(), 1);
        assert_eq!(
            run.pass_manifests[0].decisions()[0]
                .consumed_facts()
                .iter()
                .filter(|fact| matches!(
                    fact,
                    omega_optimization_core::OptimizationFactReference::AcceptedObligation(_)
                ))
                .count(),
            1
        );
        assert_eq!(run.session.unit().accepted_obligation_facts.len(), 1);
        assert_eq!(run.session.input().context().accepted_facts().len(), 1);
        assert_eq!(
            run.session.input().context().accepted_facts()[0].obligation,
            psi_core::ObligationId::new(419).unwrap()
        );
        assert!(matches!(
            run.session.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(15),
                ..
            }
        ));
    }

    #[test]
    fn public_run_rejects_a_registry_detached_from_named_selections() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let empty = OrderedRuleRegistry::new(Vec::new()).unwrap();
        assert!(matches!(
            run_psi_registry(verified_empty_unit(), &selections, &empty, budget(2)),
            Err(OptimizationRunError::SelectionRegistryMismatch)
        ));
    }
}
