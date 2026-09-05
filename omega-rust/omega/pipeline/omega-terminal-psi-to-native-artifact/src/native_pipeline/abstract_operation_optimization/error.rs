use omega_abstract_operations_optimizer::OptimizationRunError;
use omega_abstract_operations_optimizer::OptimizedAbstractProjectionError;
use omega_psi_to_abstract_operations::{
    ArtifactLoweringError, VerifiedPsiOptimizationUnitBuildError,
};

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
