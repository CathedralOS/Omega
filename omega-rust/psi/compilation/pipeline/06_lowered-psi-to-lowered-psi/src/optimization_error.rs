//! Optimizer module role: stage group. The stage's failure vocabulary.
//!
//! Every way `run_psi_optimization` can refuse: an invalid module, semantic,
//! proof bundle, debug map or execution record on either side of the selected
//! passes, and each pass's own refusal.

use terminal_codec::{CodecError, DebugMapError, PsiOptimizationExecutionRecordError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationStageError {
    InvalidControlFlowCleanupRewrite(crate::control_flow_cleanup::ControlFlowCleanupRewriteError),
    InvalidGlobalValueNumberingRewrite(
        crate::global_value_numbering::GlobalValueNumberingRewriteError,
    ),
    InvalidSparseConditionalConstantPropagationRewrite(
        crate::sparse_conditional_constant_propagation::SparseConditionalConstantPropagationRewriteError,
    ),
    InvalidSemantic(CodecError),
    InvalidDebugMap(DebugMapError),
    InvalidExecutionRecord(PsiOptimizationExecutionRecordError),
}

impl std::fmt::Display for PsiOptimizationStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidControlFlowCleanupRewrite(error) => {
                write!(formatter, "invalid control flow cleanup rewrite: {error:?}")
            }
            Self::InvalidGlobalValueNumberingRewrite(error) => {
                write!(
                    formatter,
                    "invalid global value numbering rewrite: {error:?}"
                )
            }
            Self::InvalidSparseConditionalConstantPropagationRewrite(error) => {
                write!(
                    formatter,
                    "invalid sparse conditional constant propagation rewrite: {error:?}"
                )
            }
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
