use crate::{FunctionRelativeLayoutCatalogError, OptimizedX86BranchRelaxationError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLayoutOptimizationError {
    Catalog(FunctionRelativeLayoutCatalogError),
    Relaxation(OptimizedX86BranchRelaxationError),
    CurrentProgramMismatch,
    SelectionMismatch,
}

impl std::fmt::Display for ResolvedLayoutOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "resolved-layout optimization failed: {self:?}")
    }
}
impl std::error::Error for ResolvedLayoutOptimizationError {}
