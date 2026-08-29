//! Explicit Terminal-Psi optimization request and verified projection entrance.

use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use omega_optimization_run_to_abstract_operations::{
    OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan, project_optimization_run,
};
use omega_psi_optimizer::{OptimizationRunError, run_psi_pipeline};
use omega_psi_to_abstract_operations::{
    ArtifactLoweringError, VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError,
    build_verified_psi_optimization_unit, lower_artifact_sections_for_optimization,
};
use psi_proof_admission::AdmissionProfile;

/// Exact optimizer inputs chosen by the compiler coordinator.
///
/// Construction rejects the empty selection so compatibility builds cannot
/// accidentally enter this crate or manufacture optimizer work records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitOptimizationRequest {
    selections: OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
}

impl ExplicitOptimizationRequest {
    pub fn new(
        selections: OptimizationSelections,
        budget_per_pass: OptimizationWorkBudget,
    ) -> Result<Self, EmptyOptimizationSelections> {
        if selections.is_empty() {
            return Err(EmptyOptimizationSelections);
        }
        Ok(Self {
            selections,
            budget_per_pass,
        })
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selections
    }

    pub const fn budget_per_pass(&self) -> OptimizationWorkBudget {
        self.budget_per_pass
    }
}

/// Compiler-owned bounded baseline for the experimental optimized lane.
/// Every value is a per-pass-group ceiling; this is not a source-visible
/// optimization level or an intensity preset.
pub fn compiler_baseline_request_v1(
    selections: &OptimizationSelections,
) -> Result<ExplicitOptimizationRequest, EmptyOptimizationSelections> {
    ExplicitOptimizationRequest::new(selections.clone(), compiler_baseline_budget_v1())
}

fn compiler_baseline_budget_v1() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(1_000_000, 100_000, 100_000, 100_000, 10_000)
        .expect("compiler baseline optimizer ceilings are nonzero")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyOptimizationSelections;

impl std::fmt::Display for EmptyOptimizationSelections {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("the explicit optimizer pipeline requires at least one named selection")
    }
}

impl std::error::Error for EmptyOptimizationSelections {}

#[derive(Debug)]
pub enum OptimizationPipelineError {
    ArtifactLowering(ArtifactLoweringError),
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    AbstractProjection(OptimizedAbstractProjectionError),
}

impl std::fmt::Display for OptimizationPipelineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "explicit optimization pipeline failed: {self:?}")
    }
}

impl std::error::Error for OptimizationPipelineError {}

pub fn optimize_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &AdmissionProfile,
    request: ExplicitOptimizationRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let input = lower_artifact_sections_for_optimization(semantic_bytes, proof_bytes, profile)
        .map_err(OptimizationPipelineError::ArtifactLowering)?;
    optimize_verified_psi_input(input, request)
}

pub fn optimize_verified_psi_input(
    input: VerifiedPsiOptimizationInput,
    request: ExplicitOptimizationRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    run_verified_psi_input(input, request.selections(), request.budget_per_pass())
}

/// Carry a verified Psi input through the canonical optimizer pipeline without
/// selecting a transformation. This is the compiler's no-op forwarding path,
/// not an alternate lowering route.
pub fn forward_verified_psi_input(
    input: VerifiedPsiOptimizationInput,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    run_verified_psi_input(
        input,
        &OptimizationSelections::default(),
        compiler_baseline_budget_v1(),
    )
}

fn run_verified_psi_input(
    input: VerifiedPsiOptimizationInput,
    selections: &OptimizationSelections,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let verified = build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(OptimizationPipelineError::UnitBuild)?;
    let run = run_psi_pipeline(verified, selections, budget_per_pass)
        .map_err(OptimizationPipelineError::Run)?;
    project_optimization_run(run).map_err(OptimizationPipelineError::AbstractProjection)
}
