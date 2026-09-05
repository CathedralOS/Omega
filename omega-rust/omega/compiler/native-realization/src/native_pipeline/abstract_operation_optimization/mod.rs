//! Optimizer module role: executable entrance. Verified post-Terminal abstract-operation optimization.
//!
//! [`request`] owns exact opt-in selections and bounded work. [`error`] owns
//! the closed stage failures. This entrance visibly performs artifact
//! admission followed by the complete abstract optimization phase. Unit
//! construction, pass execution and publication belong to that phase.

mod error;
mod request;

pub use error::OptimizationPipelineError;
pub use request::{
    EmptyOptimizationSelections, ExplicitOptimizationRequest, OptimizationPipelineRequest,
    compiler_baseline_request_v1,
};

use abstract_operations_to_abstract_operations::{
    AbstractOptimizationError, ValidatedOptimizedAbstractPlan, optimize_abstract_operations,
};
use proof_admission::AdmissionProfile;
use terminal_psi_to_abstract_operations::{
    VerifiedPsiOptimizationInput, lower_artifact_sections_for_optimization,
};

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
    optimize_abstract_operations(
        input,
        request.selections(),
        request.psi_projection(),
        request.budget_per_pass(),
    )
    .map_err(|error| match error {
        AbstractOptimizationError::UnitBuild(error) => OptimizationPipelineError::UnitBuild(error),
        AbstractOptimizationError::Run(error) => OptimizationPipelineError::Run(error),
        AbstractOptimizationError::Publication(error) => {
            OptimizationPipelineError::AbstractProjection(error)
        }
    })
}
