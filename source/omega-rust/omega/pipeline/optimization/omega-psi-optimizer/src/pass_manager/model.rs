use omega_optimization_core::{
    InvalidOptimizationManifestRecord, OptimizationCandidateIdentity, OptimizationIdentityBundle,
    OptimizationPassIdentity, OptimizationPassManifestRecord, OptimizationRuleIdentity,
    OptimizationSelections, OptimizationUnitIdentity, OptimizationValidatorIdentity,
    OptimizationWorkBudget,
};
use omega_optimization_policy::{
    BaselineDecisionLog, BaselineDecisionLogDecodeError, BaselineDecisionRecordError,
    ExternalDecisionLog, ExternalDecisionSchemaError,
};
use omega_optimization_unit::{
    InvalidPsiTransformationLedger, ProvenanceRewrite, PsiOptimizationUnit, PsiRewriteCandidate,
    PsiTransformationLedger,
};
use omega_optimization_validation::{
    OptimizationUnitValidationError, ValidatedOptimizerCycleComponents,
    ValidatedOptimizerRankingCertificates, validate_verified_psi_cycle_components,
};
use omega_psi_to_abstract_operations::{VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit};

use crate::{
    AnalysisManagerError, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantAnalysisSnapshot, CountdownInvariantConstantPlacementAnalysisError,
    CountdownInvariantConstantPlacementAnalysisSnapshot, CountedLoopAnalysisError,
    CountedLoopAnalysisSnapshot, RuleProposalError, RuleRegistryError,
    ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountdownInvariantConstantPlacementAnalysis, ValidatedCountedLoopAnalysis,
    analyze_countdown_invariant_constant_placement, analyze_countdown_invariant_constants,
    analyze_counted_loops, validate_countdown_invariant_constant_analysis,
    validate_countdown_invariant_constant_placement_analysis, validate_counted_loop_analysis,
};

#[derive(Debug)]
pub struct VerifiedPsiOptimizationSession {
    pub(super) input: VerifiedPsiOptimizationInput,
    pub(super) unit: PsiOptimizationUnit,
    pub(super) cycle_components: ValidatedOptimizerCycleComponents,
}

impl VerifiedPsiOptimizationSession {
    pub fn new(
        verified: VerifiedPsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        let cycle_components = validate_verified_psi_cycle_components(&verified)?;
        let (input, unit) = verified.into_parts();
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

    /// Canonical SCC topology authorized for optimizer analysis only.
    pub const fn cycle_components(&self) -> &ValidatedOptimizerCycleComponents {
        &self.cycle_components
    }

    /// Exact ranking evidence available to loop analyses, without execution
    /// or cyclic-rewrite authority.
    pub const fn ranking_certificates(&self) -> &ValidatedOptimizerRankingCertificates {
        self.cycle_components.ranking_certificates()
    }

    /// Independently reconstructed, revision-bound counted-loop facts. This
    /// grants analysis custody only, never cyclic rewrite or execution rights.
    pub fn counted_loop_analysis(
        &self,
    ) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
        analyze_counted_loops(&self.unit, &self.cycle_components)
    }

    pub fn validate_counted_loop_analysis(
        &self,
        candidate: &CountedLoopAnalysisSnapshot,
    ) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
        validate_counted_loop_analysis(&self.unit, &self.cycle_components, candidate)
    }

    /// Exact input-free integer constants owned by the authenticated countdown
    /// relation. This grants analysis custody only and cannot move a node.
    pub fn countdown_invariant_constant_analysis(
        &self,
    ) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError>
    {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantAnalysisError::CountedLoop)?;
        analyze_countdown_invariant_constants(&self.unit, &self.cycle_components, &counted)
    }

    pub fn validate_countdown_invariant_constant_analysis(
        &self,
        candidate: &CountdownInvariantConstantAnalysisSnapshot,
    ) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError>
    {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantAnalysisError::CountedLoop)?;
        validate_countdown_invariant_constant_analysis(
            &self.unit,
            &self.cycle_components,
            &counted,
            candidate,
        )
    }

    /// Exact preheader insertion and consumer coordinates for the authenticated
    /// countdown constants. This remains analysis-only and cannot move a node.
    pub fn countdown_invariant_constant_placement_analysis(
        &self,
    ) -> Result<
        ValidatedCountdownInvariantConstantPlacementAnalysis,
        CountdownInvariantConstantPlacementAnalysisError,
    > {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantPlacementAnalysisError::CountedLoop)?;
        let invariants =
            analyze_countdown_invariant_constants(&self.unit, &self.cycle_components, &counted)
                .map_err(CountdownInvariantConstantPlacementAnalysisError::InvariantConstant)?;
        analyze_countdown_invariant_constant_placement(
            &self.unit,
            &self.cycle_components,
            &counted,
            &invariants,
        )
    }

    pub fn validate_countdown_invariant_constant_placement_analysis(
        &self,
        candidate: &CountdownInvariantConstantPlacementAnalysisSnapshot,
    ) -> Result<
        ValidatedCountdownInvariantConstantPlacementAnalysis,
        CountdownInvariantConstantPlacementAnalysisError,
    > {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantPlacementAnalysisError::CountedLoop)?;
        let invariants =
            analyze_countdown_invariant_constants(&self.unit, &self.cycle_components, &counted)
                .map_err(CountdownInvariantConstantPlacementAnalysisError::InvariantConstant)?;
        validate_countdown_invariant_constant_placement_analysis(
            &self.unit,
            &self.cycle_components,
            &counted,
            &invariants,
            candidate,
        )
    }

    pub fn into_parts(self) -> (VerifiedPsiOptimizationInput, PsiOptimizationUnit) {
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

/// Full immutable declaration retained for every independently validated Psi
/// candidate, whether policy applies or skips it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsiValidatedCandidateDeclaration {
    pub pass: OptimizationPassIdentity,
    pub declaration: PsiRewriteCandidate,
    pub validator: OptimizationValidatorIdentity,
}

impl PsiValidatedCandidateDeclaration {
    pub const fn pass(&self) -> OptimizationPassIdentity {
        self.pass
    }

    pub const fn declaration(&self) -> &PsiRewriteCandidate {
        &self.declaration
    }

    pub const fn validator(&self) -> OptimizationValidatorIdentity {
        self.validator
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
    pub validated_candidates: Vec<PsiValidatedCandidateDeclaration>,
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

    pub fn validated_candidates(&self) -> &[PsiValidatedCandidateDeclaration] {
        &self.validated_candidates
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
