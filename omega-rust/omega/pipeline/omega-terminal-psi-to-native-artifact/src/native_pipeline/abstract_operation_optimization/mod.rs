//! Optimizer module role: executable entrance. Verified post-Terminal abstract-operation optimization.
//!
//! [`request`] owns exact opt-in selections and bounded work. [`error`] owns
//! the closed stage failures. This entrance visibly performs artifact
//! admission, verified-unit construction, the selected abstract pass run, and
//! independent abstract-plan projection in that order.

mod error;
mod request;

pub use error::OptimizationPipelineError;
pub use request::{
    EmptyOptimizationSelections, ExplicitOptimizationRequest, OptimizationPipelineRequest,
    compiler_baseline_request_v1,
};

use omega_abstract_operations_optimizer::run_psi_pipeline_for_projection;
use omega_optimization_run_to_abstract_operations::{
    ValidatedOptimizedAbstractPlan, project_optimization_run,
};
use omega_psi_to_abstract_operations::{
    VerifiedPsiOptimizationInput, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};
use psi_proof_admission::AdmissionProfile;

pub fn optimize_artifact_sections(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &AdmissionProfile,
    request: impl Into<OptimizationPipelineRequest>,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let input = lower_artifact_sections_for_optimization(semantic_bytes, proof_bytes, profile)
        .map_err(OptimizationPipelineError::ArtifactLowering)?;
    optimize_verified_abstract_input(input, request)
}

pub fn optimize_verified_abstract_input(
    input: VerifiedPsiOptimizationInput,
    request: impl Into<OptimizationPipelineRequest>,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let request = request.into();
    run_verified_abstract_input(input, &request)
}

fn run_verified_abstract_input(
    input: VerifiedPsiOptimizationInput,
    request: &OptimizationPipelineRequest,
) -> Result<ValidatedOptimizedAbstractPlan, OptimizationPipelineError> {
    let verified = build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(OptimizationPipelineError::UnitBuild)?;
    let run = run_psi_pipeline_for_projection(
        verified,
        request.selections(),
        request.psi_projection(),
        request.budget_per_pass(),
    )
    .map_err(OptimizationPipelineError::Run)?;
    project_optimization_run(run).map_err(OptimizationPipelineError::AbstractProjection)
}
