//! Optimizer module role: stage group. The stage's failure vocabulary.
//!
//! Every way `run_psi_optimization` can refuse: an invalid module, semantic,
//! proof bundle, debug map or execution record on either side of the selected
//! passes, and each pass's own refusal.

use terminal_codec::{
    CodecError, DebugMapError, ProofCodecError, PsiOptimizationExecutionRecordError,
};
use terminal_verifier::ModuleError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationStageError {
    InvalidControlFlowCleanupRewrite(terminal_verifier::ControlFlowCleanupRewriteError),
    InvalidCopyPropagationRewrite(terminal_verifier::CopyPropagationRewriteError),
    InvalidDeadScalarRewrite(terminal_verifier::DeadScalarRewriteError),
    InvalidGlobalValueNumberingRewrite(terminal_verifier::GlobalValueNumberingRewriteError),
    InvalidSparseConditionalConstantPropagationRewrite(
        terminal_verifier::SparseConditionalConstantPropagationRewriteError,
    ),
    InvalidProofCheckElisionRewrite(terminal_verifier::ProofCheckElisionRewriteError),
    InvalidModule(ModuleError),
    InvalidSemantic(CodecError),
    InvalidProof(ProofCodecError),
    InvalidDebugMap(DebugMapError),
    InvalidExecutionRecord(PsiOptimizationExecutionRecordError),
}

impl std::fmt::Display for PsiOptimizationStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidControlFlowCleanupRewrite(error) => {
                write!(formatter, "invalid control flow cleanup rewrite: {error:?}")
            }
            Self::InvalidCopyPropagationRewrite(error) => {
                write!(formatter, "invalid copy propagation rewrite: {error:?}")
            }
            Self::InvalidDeadScalarRewrite(error) => {
                write!(formatter, "invalid dead scalar rewrite: {error:?}")
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
            Self::InvalidProofCheckElisionRewrite(error) => {
                write!(formatter, "invalid proof check elision rewrite: {error:?}")
            }
            Self::InvalidModule(error) => write!(formatter, "invalid optimization input: {error}"),
            Self::InvalidSemantic(error) => {
                write!(formatter, "invalid optimization semantics: {error}")
            }
            Self::InvalidProof(error) => write!(formatter, "invalid optimization proof: {error}"),
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
