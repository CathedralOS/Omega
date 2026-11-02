//! Optimizer module role: stage group. The stage's failure vocabulary.
//!
//! Every way `run_psi_optimization` can refuse: an invalid module, semantic,
//! proof bundle, debug map or execution record on either side of the selected
//! passes.

use terminal_codec::{CodecError, DebugMapError, PsiOptimizationExecutionRecordError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationStageError {
    InvalidSemantic(CodecError),
    InvalidDebugMap(DebugMapError),
    InvalidExecutionRecord(PsiOptimizationExecutionRecordError),
}

impl std::fmt::Display for PsiOptimizationStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSemantic(error) => {
                write!(formatter, "invalid optimization semantics: {error}")
            }
            Self::InvalidDebugMap(error) => {
                write!(formatter, "invalid optimization debug map: {error}")
            }
            Self::InvalidExecutionRecord(error) => {
                write!(formatter, "invalid optimization execution record: {error}")
            }
        }
    }
}

impl std::error::Error for PsiOptimizationStageError {}
