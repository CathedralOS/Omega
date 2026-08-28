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
    BaselineDecisionLog, BaselineDecisionLogDecodeError, BaselineDecisionOutcome,
    BaselineDecisionRecordError, BaselinePolicy, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ExternalDecisionSchemaError,
    ValidatedCandidateSummary, external_psi_decision_schema_v1_identity,
    psi_target_neutral_decision_target_v1_identity,
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
    /// Typed, versioned policy surface recorded after ordinary candidate
    /// validation. It is not consulted by the baseline run and therefore does
    /// not alter selection, commits, manifests, ledgers, or executable output.
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

    pub fn commits(&self) -> &[PsiOptimizationCommit] {
        &self.commits
    }

    pub const fn usage(&self) -> OptimizationRunUsage {
        self.usage
    }

    pub const fn decisions(&self) -> &BaselineDecisionLog {
        &self.decisions
    }

    pub const fn external_decisions(&self) -> &ExternalDecisionLog {
        &self.external_decisions
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
    ExternalDecisionSchema(ExternalDecisionSchemaError),
    ExternalDecisionManifestMismatch,
    ExternalDecisionReplay(ExternalDecisionReplayError),
    WorkUsageOverflow,
    MissingPassManifest,
    DuplicatePipelineRule,
    RegistryConstruction(RuleRegistryError),
    SelectionRegistryMismatch,
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

impl std::fmt::Display for ExternalDecisionReplayError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "external Psi decision replay failed: {self:?}")
    }
}

impl std::error::Error for ExternalDecisionReplayError {}

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

/// Replay a canonical external decision log through one exact selected Psi
/// registry. The byte boundary is intentional: strict schema decoding is part
/// of accepting external policy input.
pub fn replay_psi_registry(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
    encoded_external_decisions: &[u8],
) -> Result<OptimizationRun, OptimizationRunError> {
    let external_decisions =
        ExternalDecisionLog::decode(encoded_external_decisions).map_err(|error| {
            OptimizationRunError::ExternalDecisionReplay(ExternalDecisionReplayError::Schema(error))
        })?;
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
        run_registries_with_external_decisions(session, selections, &[], budget, external_decisions)
    } else {
        run_registries_with_external_decisions(
            session,
            selections,
            std::slice::from_ref(registry),
            budget,
            external_decisions,
        )
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

/// Replay a canonical external decision log through the ordinary selected Psi
/// pipeline. Candidate construction and validation are identical to the
/// model-free run; the log supplies only the action after validation.
pub fn replay_psi_pipeline(
    verified: VerifiedPsiOptimizationUnit,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
    encoded_external_decisions: &[u8],
) -> Result<OptimizationRun, OptimizationRunError> {
    let external_decisions =
        ExternalDecisionLog::decode(encoded_external_decisions).map_err(|error| {
            OptimizationRunError::ExternalDecisionReplay(ExternalDecisionReplayError::Schema(error))
        })?;
    let registries =
        built_in_psi_registries(selections).map_err(OptimizationRunError::RegistryConstruction)?;
    let session = VerifiedPsiOptimizationSession::new(verified)
        .map_err(OptimizationRunError::InitialValidation)?;
    run_registries_with_external_decisions(
        session,
        selections,
        &registries,
        budget_per_pass,
        external_decisions,
    )
}

fn run_registries(
    session: VerifiedPsiOptimizationSession,
    selections: &OptimizationSelections,
    registries: &[OrderedRuleRegistry],
    budget_per_pass: OptimizationWorkBudget,
) -> Result<OptimizationRun, OptimizationRunError> {
    run_registries_inner(session, selections, registries, budget_per_pass, None)
}

fn run_registries_with_external_decisions(
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
    let terminal_psi = session.unit.terminal_psi;
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
        external_decisions,
        pass_manifests,
        transformation_ledger,
        identity_bundle,
    })
}

struct ExternalDecisionReplayCursor<'log> {
    points: &'log [ExternalDecisionPoint],
    next: usize,
}

impl<'log> ExternalDecisionReplayCursor<'log> {
    fn new(
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

    fn choose(
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
fn run_unit(
    unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
) -> Result<OptimizationRunOutput, OptimizationRunError> {
    run_unit_inner(unit, registry, budget, None)
}

fn run_unit_inner(
    mut unit: PsiOptimizationUnit,
    registry: &OrderedRuleRegistry,
    budget: OptimizationWorkBudget,
    mut external_replay: Option<&mut ExternalDecisionReplayCursor<'_>>,
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

fn proof_certified_scalar_operation_count(unit: &PsiOptimizationUnit) -> u64 {
    unit.functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                omega_terminal_abstract_operations::TerminalAbstractOperation::IntegerExactCast { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerShiftLeft { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerShiftRight { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerAdd { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerSubtract { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerMultiply { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerDivide { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::ExactIntegerRemainder { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerDivide { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::WrappingIntegerRemainder { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerDivide { .. }
                    | omega_terminal_abstract_operations::TerminalAbstractOperation::SaturatingIntegerRemainder { .. }
            )
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

fn convergence_measure(unit: &PsiOptimizationUnit, registry: &OrderedRuleRegistry) -> u64 {
    let copy_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.copy-propagation.v1",
    );
    let cfg_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.control-flow-cleanup.v11",
    );
    let dead_scalar_pass = omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
        b"omega.psi-pass.dead-pure-scalar-elimination.v2",
    );
    let proof_elision_pass =
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.proof-check-elision.v9",
        );
    let global_value_numbering_pass =
        omega_optimization_core::OptimizationPassIdentity::from_canonical_bytes(
            b"omega.psi-pass.global-value-numbering.v7",
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
        unit.functions
            .iter()
            .flat_map(|function| &function.blocks)
            .map(|block| block.nodes.len() as u64)
            .sum()
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
            SelfDividePolicy, SelfRemainderPolicy, boolean_unit, compatible_policy_local_cse_unit,
            compatible_policy_phi_translated_gvn_unit, constant_conditional_same_target_unit,
            dead_exact_add_unit, dead_wrapping_add_unit, dependent_exact_chain_unit,
            diamond_dominator_gvn_unit, dominator_gvn_unit, exact_add_unit,
            linear_empty_block_unit, live_divide_by_one_unit, live_exact_multiply_by_zero_unit,
            live_exact_self_subtract_unit, live_exact_zero_value_shift_unit, live_self_divide_unit,
            live_self_remainder_unit, local_cse_unit, non_adjacent_merge_unit,
            phi_translated_gvn_unit, proof_certified_dominator_gvn_unit,
            proof_certified_local_cse_unit, proof_certified_phi_translated_gvn_unit,
            propagated_block_parameter_unit, randomized_built_in_registries,
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

    #[derive(Debug)]
    struct InvalidEvaluationExactRule;

    impl PsiOptimizationRule for InvalidEvaluationExactRule {
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
                    let PsiRewritePatch::ReplaceIntegerOperationWithConstant(mut patch) =
                        candidate.patch()
                    else {
                        return Err(RuleProposalError::InvalidCandidate(
                            omega_optimization_unit::PsiRewriteCandidateError::PatchDecisionPointMismatch,
                        ));
                    };
                    patch.constant = psi_core::IntegerValue::Unsigned(0);
                    omega_optimization_unit::PsiRewriteCandidate::new_integer_evaluation(
                        candidate.input(),
                        Self.contract(),
                        candidate.affected_blocks().to_vec(),
                        candidate.substitutions().to_vec(),
                        candidate.provenance().to_vec(),
                        candidate.scalar_evaluation_witness().unwrap(),
                        candidate.predicted_cost_delta(),
                        patch,
                    )
                    .map_err(RuleProposalError::InvalidCandidate)
                })
                .collect()
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
            quotient_correspondences: Vec::new(),
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
        verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(8))
    }

    fn verified_exact_add_zero_unit() -> VerifiedPsiOptimizationUnit {
        verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(0))
    }

    fn verified_compatible_policy_cse_unit() -> VerifiedPsiOptimizationUnit {
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

        let machine = MachineId::new(451).unwrap();
        let block = BlockId::new(452).unwrap();
        let left = ValueId::new(453).unwrap();
        let right = ValueId::new(454).unwrap();
        let leader = ValueId::new(455).unwrap();
        let redundant = ValueId::new(456).unwrap();
        let result = ValueId::new(462).unwrap();
        let obligation = ObligationId::new(457).unwrap();
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
            quotient_correspondences: Vec::new(),
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
                            id: OperationId::new(463).unwrap(),
                            result: OperationResult::Scalar(declaration(left)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(7),
                            },
                        },
                        Operation {
                            id: OperationId::new(464).unwrap(),
                            result: OperationResult::Scalar(declaration(right)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Unsigned(8),
                            },
                        },
                        Operation {
                            id: OperationId::new(458).unwrap(),
                            result: OperationResult::Scalar(declaration(leader)),
                            kind: OperationKind::WrappingIntegerAdd { left, right },
                        },
                        Operation {
                            id: OperationId::new(459).unwrap(),
                            result: OperationResult::Scalar(declaration(redundant)),
                            kind: OperationKind::ExactIntegerAdd {
                                left: right,
                                right: left,
                                obligation,
                            },
                        },
                    ],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(460).unwrap(),
                        value: redundant,
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(461).unwrap(),
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

    fn verified_compatible_policy_phi_gvn_unit() -> VerifiedPsiOptimizationUnit {
        use psi_core::{
            BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
            ObligationId, OperationId, ScalarType, ValueId,
        };
        use psi_proof_admission::{EvidenceRoute, PrimitiveJudgment};
        use psi_terminal::{
            Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
            TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
            VocabularyMarker,
        };
        use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

        let machine = MachineId::new(501).unwrap();
        let join = BlockId::new(502).unwrap();
        let left_block = BlockId::new(503).unwrap();
        let entry = BlockId::new(504).unwrap();
        let right_block = BlockId::new(505).unwrap();
        let condition = ValueId::new(506).unwrap();
        let left_a = ValueId::new(507).unwrap();
        let left_b = ValueId::new(508).unwrap();
        let right_a = ValueId::new(509).unwrap();
        let right_b = ValueId::new(510).unwrap();
        let join_a = ValueId::new(511).unwrap();
        let join_b = ValueId::new(512).unwrap();
        let left_leader = ValueId::new(513).unwrap();
        let right_leader = ValueId::new(514).unwrap();
        let redundant = ValueId::new(515).unwrap();
        let result = ValueId::new(516).unwrap();
        let obligation = ObligationId::new(517).unwrap();
        let zero = ValueId::new(527).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let declaration = |id, scalar_type| ValueDeclaration { id, scalar_type };
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
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![
                    declaration(condition, ScalarType::Boolean),
                    declaration(left_a, scalar_type),
                    declaration(left_b, scalar_type),
                    declaration(right_a, scalar_type),
                    declaration(right_b, scalar_type),
                ],
                structural_parameters: Vec::new(),
                result: TerminalMachineResult::Scalar(declaration(result, scalar_type)),
                structural_places: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry,
                blocks: vec![
                    Block {
                        id: join,
                        parameters: vec![
                            declaration(join_a, scalar_type),
                            declaration(join_b, scalar_type),
                        ],
                        operations: vec![Operation {
                            id: OperationId::new(518).unwrap(),
                            result: OperationResult::Scalar(declaration(redundant, scalar_type)),
                            kind: OperationKind::ExactIntegerShiftLeft {
                                value: join_a,
                                count: join_b,
                                obligation,
                            },
                        }],
                        terminator: Terminator::Return {
                            cleanup_actions: Vec::new(),
                            edge: EdgeId::new(519).unwrap(),
                            value: redundant,
                        },
                    },
                    Block {
                        id: left_block,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(520).unwrap(),
                            result: OperationResult::Scalar(declaration(left_leader, scalar_type)),
                            kind: OperationKind::WrappingIntegerShiftLeft {
                                value: left_a,
                                count: zero,
                            },
                        }],
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(521).unwrap(),
                            target: join,
                            arguments: vec![left_a, zero],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                    Block {
                        id: entry,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(528).unwrap(),
                            result: OperationResult::Scalar(declaration(zero, scalar_type)),
                            kind: OperationKind::IntegerConstant {
                                value: IntegerValue::Signed(0),
                            },
                        }],
                        terminator: Terminator::Conditional {
                            condition,
                            when_true: SuccessorEdge {
                                edge: EdgeId::new(522).unwrap(),
                                target: left_block,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                            when_false: SuccessorEdge {
                                edge: EdgeId::new(523).unwrap(),
                                target: right_block,
                                arguments: Vec::new(),
                                trivial_affine_discards: Vec::new(),
                            },
                        },
                    },
                    Block {
                        id: right_block,
                        parameters: Vec::new(),
                        operations: vec![Operation {
                            id: OperationId::new(524).unwrap(),
                            result: OperationResult::Scalar(declaration(right_leader, scalar_type)),
                            kind: OperationKind::WrappingIntegerShiftLeft {
                                value: right_a,
                                count: zero,
                            },
                        }],
                        terminator: Terminator::Jump {
                            edge: EdgeId::new(525).unwrap(),
                            target: join,
                            arguments: vec![right_a, zero],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                ],
                contract: MachineContract {
                    id: ContractId::new(526).unwrap(),
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

    fn verified_exact_add_unit_with_right(
        right_constant: psi_core::IntegerValue,
    ) -> VerifiedPsiOptimizationUnit {
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
            quotient_correspondences: Vec::new(),
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
                                value: right_constant,
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

    fn verified_exact_self_division_or_remainder_unit(divide: bool) -> VerifiedPsiOptimizationUnit {
        use psi_core::{
            BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType, IntegerValue,
            MachineId, ObligationId, OperationId, Proposition, ScalarTerm, ScalarType, ValueId,
        };
        use psi_proof_admission::{
            CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
        };
        use psi_terminal::{
            Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
            TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
        };
        use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

        let machine = MachineId::new(431).unwrap();
        let block = BlockId::new(432).unwrap();
        let operand = ValueId::new(433).unwrap();
        let remainder = ValueId::new(434).unwrap();
        let result = ValueId::new(435).unwrap();
        let obligation = ObligationId::new(436).unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer);
        let declaration = |id| ValueDeclaration { id, scalar_type };
        let one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
        let goal = Proposition::LessOrEqual(one, ScalarTerm::value(operand, scalar_type));
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
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: machine,
                attachment: None,
                parameters: vec![declaration(operand)],
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
                    operations: vec![Operation {
                        id: OperationId::new(437).unwrap(),
                        result: OperationResult::Scalar(declaration(remainder)),
                        kind: if divide {
                            OperationKind::ExactIntegerDivide {
                                left: operand,
                                right: operand,
                                obligation,
                            }
                        } else {
                            OperationKind::ExactIntegerRemainder {
                                left: operand,
                                right: operand,
                                obligation,
                            }
                        },
                    }],
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(438).unwrap(),
                        value: remainder,
                    },
                }],
                contract: MachineContract {
                    id: ContractId::new(439).unwrap(),
                    crash_routes: Vec::new(),
                    requires: vec![goal.clone()],
                    ensures: Vec::new(),
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle {
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation,
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(440).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal,
                        rule: ProofRule::Assumption { index: 0 },
                    },
                }),
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

    fn verified_exact_self_remainder_unit() -> VerifiedPsiOptimizationUnit {
        verified_exact_self_division_or_remainder_unit(false)
    }

    fn verified_exact_self_divide_unit() -> VerifiedPsiOptimizationUnit {
        verified_exact_self_division_or_remainder_unit(true)
    }

    fn budget(iterations: u64) -> OptimizationWorkBudget {
        OptimizationWorkBudget::new(64, 64, 64, 64, iterations).unwrap()
    }

    fn external_log_with(
        context: ExternalDecisionContext,
        points: impl IntoIterator<Item = ExternalDecisionPoint>,
    ) -> ExternalDecisionLog {
        ExternalDecisionLog::new(context, points).unwrap()
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
        assert_eq!(manifest.ordered_rules().len(), 7);
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
        assert_eq!(usage.rule_evaluations, 13);
        assert_eq!(output.functions[0].blocks.len(), 1);
        assert_eq!(ledger.records().len(), 2);
        assert_eq!(ledger.records()[0].provenance.len(), 3);
        assert!(
            ledger.records()[0]
                .provenance
                .iter()
                .all(|row| row.disposition.is_realized())
        );
        assert_eq!(manifest.unwrap().ordered_rules().len(), 7);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_control_flow_cleanup_merges_non_adjacent_blocks_to_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::ControlFlowCleanup]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        for target_before_predecessor in [false, true] {
            let unit = non_adjacent_merge_unit(target_before_predecessor);
            let (output, commits, usage, _, manifest, ledger) =
                run_unit(unit, &registry, budget(8)).unwrap();

            assert_eq!(commits.len(), 2);
            assert_eq!(usage.commits, 2);
            assert_eq!(usage.iterations, 3);
            assert_eq!(
                usage.rule_evaluations,
                if target_before_predecessor { 21 } else { 18 }
            );
            assert_eq!(output.functions[0].blocks.len(), 3);
            assert_eq!(ledger.records().len(), 2);
            assert!(ledger.records().iter().all(|record| {
                record
                    .provenance
                    .iter()
                    .all(|row| row.disposition.is_realized())
            }));
            assert_eq!(manifest.unwrap().ordered_rules().len(), 7);

            let (second, second_commits, second_usage, _, _, second_ledger) =
                run_unit(output.clone(), &registry, budget(8)).unwrap();
            assert_eq!(second, output);
            assert!(second_commits.is_empty());
            assert_eq!(second_usage.iterations, 1);
            assert_eq!(second_usage.rule_evaluations, 7);
            assert!(second_ledger.records().is_empty());
        }
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
    fn named_proof_check_elision_reaches_an_evidence_preserving_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let unit = dead_exact_add_unit();
        let accepted_fact = unit.accepted_obligation_facts[0].identity;
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);
        assert_eq!(output.accepted_obligation_facts.len(), 1);
        assert_eq!(ledger.records().len(), 1);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 9);
        assert_eq!(
            manifest.decisions()[0].consumed_facts(),
            [OptimizationFactReference::AcceptedObligation(accepted_fact)]
        );

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_proof_check_elision_materializes_self_subtract_zero_at_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
        let unit = live_exact_self_subtract_unit(integer);
        let accepted_fact = unit.accepted_obligation_facts[0].identity;
        let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
        let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(ledger.records().len(), 1);
        assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
        assert_eq!(
            commits[0].declaration.consumed_facts(),
            [OptimizationFactReference::AcceptedObligation(accepted_fact),]
        );
        assert!(matches!(
            output.functions[0].blocks[0].nodes[0].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(0),
                ..
            }
        ));
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].provenance,
            original_provenance
        );
        assert_eq!(output.functions[0].blocks[0].nodes[0].fuel, original_fuel);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_proof_check_elision_materializes_self_remainder_zero_at_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
        let unit = live_self_remainder_unit(integer, SelfRemainderPolicy::Exact);
        let accepted_fact = unit.accepted_obligation_facts[0].identity;
        let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
        let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(ledger.records().len(), 1);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 9);
        assert_eq!(
            manifest.decisions()[0].rule(),
            crate::LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract().identity()
        );
        assert_eq!(
            commits[0].declaration.consumed_facts(),
            [OptimizationFactReference::AcceptedObligation(accepted_fact)]
        );
        assert!(matches!(
            output.functions[0].blocks[0].nodes[0].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(0),
                ..
            }
        ));
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].provenance,
            original_provenance
        );
        assert_eq!(output.functions[0].blocks[0].nodes[0].fuel, original_fuel);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_proof_check_elision_materializes_self_divide_one_at_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let integer = psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap();
        let unit = live_self_divide_unit(integer, SelfDividePolicy::Exact);
        let accepted_fact = unit.accepted_obligation_facts[0].identity;
        let original_provenance = unit.functions[0].blocks[0].nodes[0].provenance.clone();
        let original_fuel = unit.functions[0].blocks[0].nodes[0].fuel.clone();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(ledger.records().len(), 1);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 9);
        assert_eq!(
            manifest.decisions()[0].rule(),
            crate::LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity()
        );
        assert_eq!(
            commits[0].declaration.consumed_facts(),
            [OptimizationFactReference::AcceptedObligation(accepted_fact)]
        );
        assert!(matches!(
            output.functions[0].blocks[0].nodes[0].operation,
            TerminalAbstractOperation::IntegerConstant {
                value: psi_core::IntegerValue::Unsigned(1),
                ..
            }
        ));
        assert_eq!(
            output.functions[0].blocks[0].nodes[0].provenance,
            original_provenance
        );
        assert_eq!(output.functions[0].blocks[0].nodes[0].fuel, original_fuel);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_global_value_numbering_reaches_a_cross_block_ledger_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(diamond_dominator_gvn_unit(), &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(usage.iterations, 3);
        assert_eq!(usage.rule_evaluations, 15);
        assert_eq!(usage.candidates, 2);
        assert_eq!(usage.validation_steps, 2);
        assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
        assert_eq!(ledger.records().len(), 2);
        assert_eq!(ledger.records()[0].provenance.len(), 5);
        assert_eq!(ledger.records()[1].provenance.len(), 4);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert_eq!(second_usage.rule_evaluations, 9);
        assert!(second_ledger.records().is_empty());
        assert_eq!(second_ledger.input(), second_ledger.output());
    }

    #[test]
    fn named_global_value_numbering_reaches_a_phi_translated_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(phi_translated_gvn_unit(), &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.rule_evaluations, 14);
        assert_eq!(usage.candidates, 1);
        assert_eq!(usage.validation_steps, 1);
        assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
        assert_eq!(ledger.records().len(), 1);
        let join = &output.functions[0].blocks[0];
        assert_eq!(join.parameters.len(), 2);
        assert_eq!(join.nodes.len(), 1);

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert_eq!(second_usage.rule_evaluations, 9);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_global_value_numbering_records_proof_phi_fact_consumption() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let unit = proof_certified_phi_translated_gvn_unit();
        let redundant_fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == psi_core::OperationId::new(1_713).unwrap())
            .unwrap()
            .identity;
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.rule_evaluations, 15);
        assert_eq!(usage.candidates, 1);
        assert_eq!(ledger.records().len(), 1);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 9);
        assert_eq!(
            manifest.decisions()[0].consumed_facts(),
            [OptimizationFactReference::AcceptedObligation(
                redundant_fact
            )]
        );
        assert_eq!(output.accepted_obligation_facts.len(), 3);
        assert_eq!(output.functions[0].blocks[0].parameters.len(), 2);

        let (_, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output, &registry, budget(8)).unwrap();
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.rule_evaluations, 9);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_global_value_numbering_records_proof_certified_fact_consumption() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let unit = proof_certified_local_cse_unit();
        let redundant_fact = unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == psi_core::OperationId::new(1_309).unwrap())
            .unwrap()
            .identity;

        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);
        assert_eq!(ledger.records().len(), 1);
        let manifest = manifest.unwrap();
        assert_eq!(manifest.ordered_rules().len(), 9);
        assert_eq!(
            manifest.decisions()[0].consumed_facts(),
            [OptimizationFactReference::AcceptedObligation(
                redundant_fact
            )]
        );

        let (second, second_commits, second_usage, _, _, second_ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second.identity, output.identity);
        assert!(second_commits.is_empty());
        assert_eq!(second_usage.iterations, 1);
        assert!(second_ledger.records().is_empty());
    }

    #[test]
    fn named_global_value_numbering_reaches_compatible_policy_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let unit = compatible_policy_local_cse_unit();
        let accepted_catalog = unit.accepted_obligation_facts.clone();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.validation_steps, 1);
        assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
        assert_eq!(ledger.records().len(), 1);
        assert_eq!(output.accepted_obligation_facts, accepted_catalog);
        assert_eq!(output.functions[0].blocks[0].nodes.len(), 3);

        let (second, commits, usage, _, _, ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second, output);
        assert!(commits.is_empty());
        assert_eq!(usage.rule_evaluations, 9);
        assert!(ledger.records().is_empty());
    }

    #[test]
    fn named_global_value_numbering_reaches_compatible_policy_phi_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let unit = compatible_policy_phi_translated_gvn_unit();
        let accepted_catalog = unit.accepted_obligation_facts.clone();
        let (output, commits, usage, _, manifest, ledger) =
            run_unit(unit, &registry, budget(8)).unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(usage.iterations, 2);
        assert_eq!(usage.rule_evaluations, 18);
        assert_eq!(usage.validation_steps, 1);
        assert_eq!(manifest.unwrap().ordered_rules().len(), 9);
        assert_eq!(ledger.records().len(), 1);
        assert_eq!(output.accepted_obligation_facts, accepted_catalog);
        assert_eq!(output.functions[0].blocks[0].parameters.len(), 2);

        let (second, commits, usage, _, _, ledger) =
            run_unit(output.clone(), &registry, budget(8)).unwrap();
        assert_eq!(second, output);
        assert!(commits.is_empty());
        assert_eq!(usage.rule_evaluations, 9);
        assert!(ledger.records().is_empty());
    }

    #[test]
    fn shuffled_builtin_registration_constructs_identical_multi_rule_runs() {
        for (optimization, unit) in [
            (
                Optimization::SparseConditionalConstantPropagation,
                dependent_exact_chain_unit(),
            ),
            (
                Optimization::ControlFlowCleanup,
                propagated_block_parameter_unit(true),
            ),
            (
                Optimization::GlobalValueNumbering,
                compatible_policy_local_cse_unit(),
            ),
            (
                Optimization::ProofCheckElision,
                live_self_divide_unit(
                    psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                    SelfDividePolicy::Exact,
                ),
            ),
            (
                Optimization::DeadPureScalarElimination,
                dead_wrapping_add_unit(),
            ),
        ] {
            let selections = OptimizationSelections::new([optimization]).unwrap();
            let expected_registry = built_in_psi_registry(&selections).unwrap();
            let expected = run_unit(unit.clone(), &expected_registry, budget(8)).unwrap();

            for (seed, registry) in randomized_built_in_registries(optimization)
                .into_iter()
                .enumerate()
            {
                let actual = run_unit(unit.clone(), &registry, budget(8)).unwrap();
                let context = format!("{optimization:?} registration seed {}", seed + 1);
                assert_eq!(actual.0, expected.0, "final unit differs for {context}");
                assert_eq!(actual.1, expected.1, "commits differ for {context}");
                assert_eq!(actual.2, expected.2, "usage differs for {context}");
                assert_eq!(actual.3, expected.3, "decisions differ for {context}");
                assert_eq!(actual.4, expected.4, "manifest differs for {context}");
                assert_eq!(actual.5, expected.5, "ledger differs for {context}");
            }
        }
    }

    #[test]
    fn full_sccp_cfg_copy_gvn_proof_dead_scalar_second_sweep_is_a_composed_ledger_fixed_point() {
        let selections = OptimizationSelections::new([
            Optimization::SparseConditionalConstantPropagation,
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
            Optimization::GlobalValueNumbering,
            Optimization::ProofCheckElision,
            Optimization::DeadPureScalarElimination,
        ])
        .unwrap();
        let registries = built_in_psi_registries(&selections).unwrap();

        for initial in [
            dependent_exact_chain_unit(),
            redundant_block_parameter_unit(true),
            dead_wrapping_add_unit(),
            dead_exact_add_unit(),
            local_cse_unit(),
            dominator_gvn_unit(),
            proof_certified_local_cse_unit(),
            proof_certified_dominator_gvn_unit(),
            live_divide_by_one_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                |psi_operation, obligation, result, scalar_type, left, right| {
                    TerminalAbstractOperation::ExactIntegerDivide {
                        psi_operation,
                        obligation,
                        result,
                        scalar_type,
                        left,
                        right,
                    }
                },
            ),
            live_exact_multiply_by_zero_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                false,
            ),
            live_exact_zero_value_shift_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                true,
            ),
            live_self_remainder_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                SelfRemainderPolicy::Exact,
            ),
            live_self_divide_unit(
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
                SelfDividePolicy::Exact,
            ),
        ] {
            let (first_output, first_manifests, first_ledger) =
                run_test_pipeline(initial, &registries);
            assert_eq!(first_manifests.len(), 6);
            assert_eq!(first_manifests[0].input(), first_ledger.input());
            assert_eq!(first_manifests[0].output(), first_manifests[1].input());
            assert_eq!(first_manifests[1].output(), first_manifests[2].input());
            assert_eq!(first_manifests[2].output(), first_manifests[3].input());
            assert_eq!(first_manifests[3].output(), first_manifests[4].input());
            assert_eq!(first_manifests[4].output(), first_manifests[5].input());
            assert_eq!(first_manifests[5].output(), first_ledger.output());
            assert!(!first_ledger.records().is_empty());

            let (second_output, second_manifests, second_delta) =
                run_test_pipeline(first_output.clone(), &registries);
            assert_eq!(second_output, first_output);
            assert_eq!(second_manifests.len(), 6);
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
        assert!(run.external_decisions().points().is_empty());
        assert_eq!(
            ExternalDecisionLog::decode(&run.external_decisions().encode()),
            Ok(run.external_decisions().clone())
        );
        assert_eq!(
            run.external_decisions().context().source(),
            run.transformation_ledger.input()
        );
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
        assert!(run.external_decisions().points().is_empty());
        assert_eq!(
            run.external_decisions().context().selections(),
            selections.identity()
        );
        assert_eq!(
            run.external_decisions().context().phase_selections(),
            run.psi_selections().identity()
        );
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
        assert_eq!(run.external_decisions().points().len(), 1);
        let external = &run.external_decisions().points()[0];
        let baseline = &run.decisions().records[0];
        assert_eq!(external.input(), baseline.input);
        assert_eq!(external.action(), baseline.outcome.into());
        assert_eq!(external.legal_candidates().len(), baseline.considered.len());
        assert_eq!(external.rule(), run.pass_manifests[0].decisions()[0].rule());
        assert_eq!(
            run.identity_bundle.decision_log(),
            Some(run.decisions().identity)
        );
        assert_ne!(
            run.external_decisions().identity(),
            run.decisions().identity
        );
        assert_eq!(
            ExternalDecisionLog::decode(&run.external_decisions().encode()),
            Ok(run.external_decisions().clone())
        );
    }

    #[test]
    fn external_decision_recording_rejects_detached_valid_context() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let mut run = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
        let empty = run_psi_pipeline(
            verified_empty_unit(),
            &OptimizationSelections::default(),
            budget(2),
        )
        .unwrap();
        run.external_decisions = empty.external_decisions;

        assert_eq!(
            validate_external_decision_recording(&run),
            Err(OptimizationRunError::ExternalDecisionManifestMismatch)
        );
    }

    #[test]
    fn external_decision_replay_preserves_the_complete_baseline_run() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
        let encoded = baseline.external_decisions().encode();
        let replayed =
            replay_psi_pipeline(verified_exact_add_unit(), &selections, budget(8), &encoded)
                .unwrap();

        assert_eq!(replayed.session.unit(), baseline.session.unit());
        assert_eq!(replayed.commits, baseline.commits);
        assert_eq!(replayed.usage, baseline.usage);
        assert_eq!(replayed.decisions, baseline.decisions);
        assert_eq!(replayed.external_decisions, baseline.external_decisions);
        assert_eq!(replayed.pass_manifests, baseline.pass_manifests);
        assert_eq!(
            replayed.transformation_ledger,
            baseline.transformation_ledger
        );
        assert_eq!(replayed.identity_bundle, baseline.identity_bundle);
        validate_external_decision_recording(&replayed).unwrap();
    }

    #[test]
    fn external_decision_record_and_replay_preserve_self_remainder_validation() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let baseline =
            run_psi_pipeline(verified_exact_self_remainder_unit(), &selections, budget(8)).unwrap();
        let [point] = baseline.external_decisions().points() else {
            panic!("self-remainder fixture has one external decision point");
        };
        assert_eq!(
            point.rule(),
            crate::LiveProofCertifiedIntegerSelfRemainderEliminationRule::contract().identity()
        );
        assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
        assert_eq!(baseline.usage().validation_steps, 1);

        let replayed = replay_psi_pipeline(
            verified_exact_self_remainder_unit(),
            &selections,
            budget(8),
            &baseline.external_decisions().encode(),
        )
        .unwrap();
        assert_eq!(replayed.session().unit(), baseline.session().unit());
        assert_eq!(replayed.commits(), baseline.commits());
        assert_eq!(replayed.decisions(), baseline.decisions());
        assert_eq!(replayed.external_decisions(), baseline.external_decisions());
        assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
        assert_eq!(
            replayed.transformation_ledger(),
            baseline.transformation_ledger()
        );
        assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
        assert_eq!(replayed.usage().validation_steps, 1);
        validate_external_decision_recording(&replayed).unwrap();
    }

    #[test]
    fn external_decision_record_and_replay_preserve_self_divide_validation() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let baseline =
            run_psi_pipeline(verified_exact_self_divide_unit(), &selections, budget(8)).unwrap();
        let [point] = baseline.external_decisions().points() else {
            panic!("self-divide fixture has one external decision point");
        };
        assert_eq!(
            point.rule(),
            crate::LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity()
        );
        assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));
        assert_eq!(baseline.usage().validation_steps, 1);

        let replayed = replay_psi_pipeline(
            verified_exact_self_divide_unit(),
            &selections,
            budget(8),
            &baseline.external_decisions().encode(),
        )
        .unwrap();
        assert_eq!(replayed.session().unit(), baseline.session().unit());
        assert_eq!(replayed.commits(), baseline.commits());
        assert_eq!(replayed.decisions(), baseline.decisions());
        assert_eq!(replayed.external_decisions(), baseline.external_decisions());
        assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
        assert_eq!(
            replayed.transformation_ledger(),
            baseline.transformation_ledger()
        );
        assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
        assert_eq!(replayed.usage().validation_steps, 1);
        validate_external_decision_recording(&replayed).unwrap();
    }

    #[test]
    fn external_decision_record_and_replay_preserve_compatible_policy_gvn() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let baseline = run_psi_pipeline(
            verified_compatible_policy_cse_unit(),
            &selections,
            budget(8),
        )
        .unwrap();
        let [point] = baseline.external_decisions().points() else {
            panic!("compatible-policy fixture has one external decision point");
        };
        assert_eq!(
            point.rule(),
            crate::SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity()
        );
        assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));

        let replayed = replay_psi_pipeline(
            verified_compatible_policy_cse_unit(),
            &selections,
            budget(8),
            &baseline.external_decisions().encode(),
        )
        .unwrap();
        assert_eq!(replayed.session().unit(), baseline.session().unit());
        assert_eq!(replayed.commits(), baseline.commits());
        assert_eq!(replayed.decisions(), baseline.decisions());
        assert_eq!(replayed.external_decisions(), baseline.external_decisions());
        assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
        assert_eq!(
            replayed.transformation_ledger(),
            baseline.transformation_ledger()
        );
        validate_external_decision_recording(&replayed).unwrap();
    }

    #[test]
    fn external_decision_record_and_replay_preserve_compatible_policy_phi_gvn() {
        let selections = OptimizationSelections::new([Optimization::GlobalValueNumbering]).unwrap();
        let baseline = run_psi_pipeline(
            verified_compatible_policy_phi_gvn_unit(),
            &selections,
            budget(8),
        )
        .unwrap();
        let [point] = baseline.external_decisions().points() else {
            panic!("compatible-policy phi fixture has one external decision point");
        };
        assert_eq!(
            point.rule(),
            crate::PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule::contract().identity()
        );
        assert!(matches!(point.action(), ExternalDecisionAction::Choose(_)));

        let replayed = replay_psi_pipeline(
            verified_compatible_policy_phi_gvn_unit(),
            &selections,
            budget(8),
            &baseline.external_decisions().encode(),
        )
        .unwrap();
        assert_eq!(replayed.session().unit(), baseline.session().unit());
        assert_eq!(replayed.commits(), baseline.commits());
        assert_eq!(replayed.decisions(), baseline.decisions());
        assert_eq!(replayed.external_decisions(), baseline.external_decisions());
        assert_eq!(replayed.pass_manifests(), baseline.pass_manifests());
        assert_eq!(
            replayed.transformation_ledger(),
            baseline.transformation_ledger()
        );
        validate_external_decision_recording(&replayed).unwrap();
    }

    #[test]
    fn external_decision_replay_supports_the_exact_registry_entry_point() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let registry = built_in_psi_registry(&selections).unwrap();
        let baseline =
            run_psi_registry(verified_exact_add_unit(), &selections, &registry, budget(8)).unwrap();
        let replayed = replay_psi_registry(
            verified_exact_add_unit(),
            &selections,
            &registry,
            budget(8),
            &baseline.external_decisions().encode(),
        )
        .unwrap();

        assert_eq!(replayed.session().unit(), baseline.session().unit());
        assert_eq!(replayed.decisions(), baseline.decisions());
        assert_eq!(replayed.external_decisions(), baseline.external_decisions());
        assert_eq!(replayed.identity_bundle(), baseline.identity_bundle());
    }

    #[test]
    fn external_skip_can_override_the_baseline_choice() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
        let [point] = baseline.external_decisions().points() else {
            panic!("exact-add fixture has one decision point");
        };
        let skipped = ExternalDecisionPoint::new(
            point.input(),
            point.rule(),
            point.legal_candidates().iter().copied(),
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
        )
        .unwrap();
        let external = external_log_with(baseline.external_decisions().context(), [skipped]);
        let replayed = replay_psi_pipeline(
            verified_exact_add_unit(),
            &selections,
            budget(8),
            &external.encode(),
        )
        .unwrap();

        assert!(replayed.commits().is_empty());
        assert_eq!(
            replayed.transformation_ledger().input(),
            replayed.transformation_ledger().output()
        );
        assert_eq!(replayed.external_decisions(), &external);
        assert_eq!(
            replayed.decisions().records[0].outcome,
            BaselineDecisionOutcome::Skip(OptimizationReasonCode::NotProfitable)
        );
        assert_eq!(replayed.usage().validation_steps, 1);
        validate_external_decision_recording(&replayed).unwrap();
    }

    #[test]
    fn external_replay_preflights_every_context_axis() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
        let context = baseline.external_decisions().context();
        let contexts = [
            (
                ExternalDecisionContext::new(
                omega_optimization_core::OptimizationDecisionSchemaIdentity::from_canonical_bytes(
                    b"foreign schema",
                ),
                context.source(),
                context.selections(),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                context.cost_model(),
                ),
                ExternalDecisionContextAxis::Schema,
            ),
            (
                ExternalDecisionContext::new(
                context.schema(),
                OptimizationUnitIdentity::from_canonical_bytes(b"foreign source"),
                context.selections(),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                context.cost_model(),
                ),
                ExternalDecisionContextAxis::Source,
            ),
            (
                ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([7; 32]),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                context.cost_model(),
                ),
                ExternalDecisionContextAxis::Selections,
            ),
            (
                ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                omega_optimization_core::OptimizationSelectionIdentity::from_bytes([8; 32]),
                context.target(),
                context.rule_set(),
                context.cost_model(),
                ),
                ExternalDecisionContextAxis::PhaseSelections,
            ),
            (
                ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                context.phase_selections(),
                omega_optimization_core::OptimizationDecisionTargetIdentity::from_canonical_bytes(
                    b"foreign target",
                ),
                context.rule_set(),
                context.cost_model(),
                ),
                ExternalDecisionContextAxis::Target,
            ),
            (
                ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                context.phase_selections(),
                context.target(),
                OptimizationRuleSetIdentity::from_canonical_bytes(b"foreign rules"),
                context.cost_model(),
                ),
                ExternalDecisionContextAxis::RuleSet,
            ),
            (
                ExternalDecisionContext::new(
                context.schema(),
                context.source(),
                context.selections(),
                context.phase_selections(),
                context.target(),
                context.rule_set(),
                TargetCostModelIdentity::from_canonical_bytes(b"foreign cost model"),
                ),
                ExternalDecisionContextAxis::CostModel,
            ),
        ];

        for (supplied, expected_axis) in contexts {
            let external = external_log_with(
                supplied,
                baseline.external_decisions().points().iter().cloned(),
            );
            assert!(matches!(
                replay_psi_pipeline(
                    verified_exact_add_unit(),
                    &selections,
                    budget(8),
                    &external.encode(),
                ),
                Err(OptimizationRunError::ExternalDecisionReplay(
                    ExternalDecisionReplayError::ContextMismatch(axis)
                )) if axis == expected_axis
            ));
        }
    }

    #[test]
    fn external_replay_rejects_missing_illegal_duplicate_and_leftover_decisions() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();
        let context = baseline.external_decisions().context();
        let point = baseline.external_decisions().points()[0].clone();

        let missing = external_log_with(context, []);
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &missing.encode(),
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::MissingDecision { .. }
            ))
        ));

        let mut wrong_cost = point.legal_candidates().to_vec();
        wrong_cost[0].predicted_cost_delta += 1;
        let illegal_point =
            ExternalDecisionPoint::new(point.input(), point.rule(), wrong_cost, point.action())
                .unwrap();
        let illegal = external_log_with(context, [illegal_point]);
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &illegal.encode(),
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::IllegalDecision { .. }
            ))
        ));

        let competing = ExternalDecisionPoint::new(
            point.input(),
            point.rule(),
            point.legal_candidates().iter().copied(),
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
        )
        .unwrap();
        let duplicate = external_log_with(context, [point.clone(), competing]);
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &duplicate.encode(),
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::DuplicateDecision { .. }
            ))
        ));

        let empty_baseline = run_psi_pipeline(
            verified_empty_unit(),
            &OptimizationSelections::default(),
            budget(2),
        )
        .unwrap();
        let unreachable = ExternalDecisionPoint::new(
            OptimizationUnitIdentity::from_canonical_bytes(b"unreachable input"),
            OptimizationRuleIdentity::from_canonical_bytes(b"unreachable rule"),
            [ValidatedCandidateSummary {
                candidate: OptimizationCandidateIdentity::from_canonical_bytes(
                    b"unreachable candidate",
                ),
                predicted_cost_delta: -1,
            }],
            ExternalDecisionAction::Skip(OptimizationReasonCode::NotProfitable),
        )
        .unwrap();
        let leftover =
            external_log_with(empty_baseline.external_decisions().context(), [unreachable]);
        assert!(matches!(
            replay_psi_pipeline(
                verified_empty_unit(),
                &OptimizationSelections::default(),
                budget(2),
                &leftover.encode(),
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::LeftoverDecisions { remaining: 1, .. }
            ))
        ));
    }

    #[test]
    fn external_replay_byte_boundary_rejects_exact_duplicate_and_foreign_action() {
        let selections =
            OptimizationSelections::new([Optimization::SparseConditionalConstantPropagation])
                .unwrap();
        let baseline = run_psi_pipeline(verified_exact_add_unit(), &selections, budget(8)).unwrap();

        let mut duplicated = baseline.external_decisions().encode();
        const LOG_POINT_COUNT_OFFSET: usize = 8 + 4 + 32 + 7 * 32;
        const LOG_POINTS_OFFSET: usize = LOG_POINT_COUNT_OFFSET + 4;
        let framed_point = duplicated[LOG_POINTS_OFFSET..].to_vec();
        duplicated[LOG_POINT_COUNT_OFFSET..LOG_POINTS_OFFSET].copy_from_slice(&2_u32.to_le_bytes());
        duplicated.extend_from_slice(&framed_point);
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &duplicated,
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::Schema(
                    ExternalDecisionSchemaError::DuplicateDecisionPoint
                )
            ))
        ));

        let mut foreign_action = baseline.external_decisions().encode();
        const POINT_BODY_OFFSET: usize = LOG_POINTS_OFFSET + 4;
        const ACTION_CANDIDATE_OFFSET: usize =
            POINT_BODY_OFFSET + 8 + 4 + 32 + 32 + 32 + 4 + 40 + 1;
        foreign_action[ACTION_CANDIDATE_OFFSET..ACTION_CANDIDATE_OFFSET + 32].copy_from_slice(
            &OptimizationCandidateIdentity::from_canonical_bytes(b"foreign").bytes(),
        );
        assert!(matches!(
            replay_psi_pipeline(
                verified_exact_add_unit(),
                &selections,
                budget(8),
                &foreign_action,
            ),
            Err(OptimizationRunError::ExternalDecisionReplay(
                ExternalDecisionReplayError::Schema(ExternalDecisionSchemaError::IllegalAction)
            ))
        ));
    }

    #[test]
    fn external_policy_input_cannot_bypass_candidate_validation() {
        let unit = exact_add_unit();
        let registry = OrderedRuleRegistry::new([
            Arc::new(InvalidEvaluationExactRule) as Arc<dyn PsiOptimizationRule>
        ])
        .unwrap();
        let mut analyses = AnalysisManager::new(&unit);
        let products = analyses
            .require_all(
                &unit,
                InvalidEvaluationExactRule.contract().required_analyses(),
            )
            .unwrap()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let candidate = InvalidEvaluationExactRule
            .propose(&unit, RuleAnalysisView::new(&products))
            .unwrap()
            .remove(0);
        let rule_set =
            OptimizationRuleSetIdentity::from_ordered_rules(&[InvalidEvaluationExactRule
                .contract()
                .identity()])
            .unwrap();
        let context = ExternalDecisionContext::new(
            external_psi_decision_schema_v1_identity(),
            unit.identity,
            OptimizationSelections::default().identity(),
            OptimizationSelections::default().identity(),
            psi_target_neutral_decision_target_v1_identity(),
            rule_set,
            baseline_psi_cost_model_identity(),
        );
        let point = ExternalDecisionPoint::new(
            unit.identity,
            candidate.rule(),
            [ValidatedCandidateSummary {
                candidate: candidate.identity(),
                predicted_cost_delta: candidate.predicted_cost_delta(),
            }],
            ExternalDecisionAction::Choose(candidate.identity()),
        )
        .unwrap();
        let log = ExternalDecisionLog::new(context, [point]).unwrap();
        let mut cursor = ExternalDecisionReplayCursor::new(&log, context).unwrap();

        assert!(matches!(
            run_unit_inner(unit, &registry, budget(2), Some(&mut cursor)),
            Err(OptimizationRunError::CandidateValidation(
                OptimizationUnitValidationError::CandidateEvaluationMismatch
            ))
        ));
        assert_eq!(
            cursor.next, 0,
            "invalid candidate did not consume policy input"
        );
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
    fn public_run_elides_live_proof_certified_identity_and_reaches_fixed_point() {
        let selections = OptimizationSelections::new([Optimization::ProofCheckElision]).unwrap();
        let run = run_psi_pipeline(verified_exact_add_zero_unit(), &selections, budget(8)).unwrap();

        assert_eq!(run.commits.len(), 1);
        assert_eq!(run.pass_manifests.len(), 1);
        assert_eq!(run.pass_manifests[0].ordered_rules().len(), 9);
        assert_eq!(run.pass_manifests[0].decisions().len(), 1);
        assert_eq!(
            run.pass_manifests[0].decisions()[0].consumed_facts().len(),
            2
        );
        assert_eq!(run.session.unit().accepted_obligation_facts.len(), 1);
        assert!(run.session.unit().functions[0].facts.iter().all(|fact| {
            !matches!(
                fact,
                omega_optimization_unit::OptimizationFact::OperationObligationReference { .. }
            )
        }));
        assert!(matches!(
            run.session.unit().functions[0].blocks[0].nodes[2].operation,
            TerminalAbstractOperation::Return { value, .. }
                if value == psi_core::ValueId::new(413).unwrap()
        ));

        let registry = built_in_psi_registry(&selections).unwrap();
        let (output, commits, usage, _, _, ledger) =
            run_unit(run.session.unit().clone(), &registry, budget(8)).unwrap();
        assert_eq!(output.identity, run.session.unit().identity);
        assert!(commits.is_empty());
        assert_eq!(usage.iterations, 1);
        assert!(ledger.records().is_empty());
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
