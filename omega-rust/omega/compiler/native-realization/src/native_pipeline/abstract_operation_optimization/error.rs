use abstract_operations_to_abstract_operations::OptimizationRunError;
use abstract_operations_to_abstract_operations::OptimizedAbstractProjectionError;
use terminal_psi_to_abstract_operations::{
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
