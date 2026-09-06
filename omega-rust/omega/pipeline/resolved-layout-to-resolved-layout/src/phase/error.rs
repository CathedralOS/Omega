use crate::{FunctionRelativeLayoutCatalogError, OptimizedX86BranchRelaxationError};
use selected_form_encoding_to_resolved_layout::OptimizedResolvedSelectedFormLayoutError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLayoutOptimizationError {
    Baseline(OptimizedResolvedSelectedFormLayoutError),
    Catalog(FunctionRelativeLayoutCatalogError),
    Relaxation(OptimizedX86BranchRelaxationError),
    UnsupportedComposition,
    CurrentProgramMismatch,
    SelectionMismatch,
}

impl std::fmt::Display for ResolvedLayoutOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "resolved-layout optimization failed: {self:?}")
    }
}
impl std::error::Error for ResolvedLayoutOptimizationError {}
