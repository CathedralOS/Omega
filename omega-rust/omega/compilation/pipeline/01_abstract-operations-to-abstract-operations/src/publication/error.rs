//! Typed rejection vocabulary for the projection boundary.

use crate::validation::PrePhysicalOptimizationManifestError;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedAbstractProjectionError {
    FunctionRosterMismatch,
    InvalidFunctionParameter { machine: MachineId, position: usize },
    InvalidBlockParameter { machine: MachineId, position: usize },
    OperationOffsetOverflow(MachineId),
    PsiSelectionProjectionMismatch,
    PrePhysicalManifest(PrePhysicalOptimizationManifestError),
}

failed_display!(
    OptimizedAbstractProjectionError,
    "cannot project optimized abstract plan"
);
