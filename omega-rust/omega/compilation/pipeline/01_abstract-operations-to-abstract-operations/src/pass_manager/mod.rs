//! Optimizer module role: stage group. Deterministic execution of exact selected Psi optimization passes.
//!
//! The run and replay entries live in `abstract_optimization.rs`; they open a
//! [`VerifiedPsiOptimizationSession`] and hand it to `run_registries` here.
//! This file owns the run carrier ([`OptimizationRun`]) and the closed error
//! surface ([`OptimizationRunError`]), [`execution`] owns candidate dispatch
//! and external-decision replay, and [`accounting`] owns convergence and
//! manifests.

mod accounting;
mod baseline;
mod execution;
mod external_policy;

use optimization_core::TargetCostModelIdentity;

use crate::ranked_rewrites::LoopInvariantScalarMotionError;
use crate::validation::ValidatedOptimizerRankingCertificates;
use crate::validation::{
    ValidatedOptimizerCycleComponents, validate_transformed_psi_cycle_components,
    validate_verified_psi_cycle_components,
};
use crate::{AnalysisManagerError, RuleProposalError, RuleRegistryError};
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
use terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnit,
};

/// Identity of the deterministic structural cost policy used by every current
/// target-neutral Psi pass.
pub(crate) fn baseline_psi_cost_model_identity() -> TargetCostModelIdentity {
    TargetCostModelIdentity::from_canonical_bytes(b"omega.psi-baseline-structural-cost-model.v1")
}

/// The verdict a baseline outcome gives one validated candidate: the chosen
/// identity applies, every other candidate is superseded, and a skip reason
/// maps onto the same skip verdict. Shared by candidate dispatch and the
/// external-decision replay check.
pub(super) fn baseline_outcome_verdict(
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
    /// Validated at construction in every build, so malformed cycle topology
    /// rejects the session; the relocation boundary reads it back per step.
    cycle_components: ValidatedOptimizerCycleComponents,
}

impl VerifiedPsiOptimizationSession {
    pub fn new(
        verified: VerifiedPsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        let cycle_components = validate_verified_psi_cycle_components(&verified)?;
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

    /// Rebinds analysis custody to an independently validated transformed
    /// revision. This grants no transformation authority.
    pub(crate) fn from_transformed(
        input: VerifiedPsiOptimizationInput,
        unit: PsiOptimizationUnit,
    ) -> Result<Self, OptimizationUnitValidationError> {
        let cycle_components = validate_transformed_psi_cycle_components(&input, &unit)?;
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

/// Cycle-component and countdown loop analyses over the session, consumed by
/// the relocation boundary in every build.
impl VerifiedPsiOptimizationSession {
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
    pub(crate) fn counted_loop_analysis(
        &self,
    ) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
        analyze_counted_loops(&self.unit, &self.cycle_components)
    }

    /// Exact input-free integer constants owned by the authenticated countdown
    /// relation. This grants analysis custody only and cannot move a node.
    pub(crate) fn countdown_invariant_constant_analysis(
        &self,
    ) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError>
    {
        let counted = self
            .counted_loop_analysis()
            .map_err(CountdownInvariantConstantAnalysisError::CountedLoop)?;
        analyze_countdown_invariant_constants(&self.unit, &self.cycle_components, &counted)
    }

    /// The countdown analysis's inputs: the counted loops and the invariant
    /// constants they carry.
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

    /// Exact preheader insertion and consumer coordinates for the authenticated
    /// countdown constants. This remains analysis-only and cannot move a node.
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

impl std::error::Error for ExternalDecisionReplayError {}
failed_display!(OptimizationRunError, "Psi optimization run failed");

impl std::error::Error for OptimizationRunError {}
