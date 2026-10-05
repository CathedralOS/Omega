//! Typed rejection vocabulary for the projection boundary.

use crate::validation::PrePhysicalOptimizationManifestError;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAbstractProjectionError {
    FunctionRosterMismatch,
    InvalidFunctionParameter {
        machine: MachineId,
        position: usize,
    },
    InvalidBlockParameter {
        machine: MachineId,
        position: usize,
    },
    OperationOffsetOverflow(MachineId),
    PsiSelectionProjectionMismatch,
    PrePhysicalManifest(PrePhysicalOptimizationManifestError),
}

impl std::fmt::Display for OptimizedAbstractProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot project optimized abstract plan: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedAbstractProjectionError {}

