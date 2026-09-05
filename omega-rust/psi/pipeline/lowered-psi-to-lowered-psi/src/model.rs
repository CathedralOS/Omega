//! Pre-Terminal optimization stage carriers and errors.

use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_codec::{
    CodecError, DebugMapError, ProofCodecError, PsiOptimizationExecutionRecord,
    PsiOptimizationExecutionRecordError,
};
use terminal_verifier::ModuleError;

use lowered_psi::LoweredPsi;

/// Validated output of the selected target-neutral Psi optimization phase.
///
/// Terminal publication accepts this type rather than an unvalidated lowering
/// result. Empty selection is an executed identity transformation. A selected
/// pass has no route until its rewrite and independent validator operate on
/// this complete carrier, including proof, debug, and source-custody sidecars.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "Terminal publication requires the validated Psi optimization result"]
pub struct PsiOptimizationStageResult {
    lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
    execution: PsiOptimizationExecutionRecord,
}

impl PsiOptimizationStageResult {
    pub(super) const fn new(
        lowered: LoweredPsi,
        selections: PsiOptimizationSelections,
        execution: PsiOptimizationExecutionRecord,
    ) -> Self {
        Self {
            lowered,
            selections,
            execution,
        }
    }

    pub const fn lowered(&self) -> &LoweredPsi {
        &self.lowered
    }

    pub const fn selections(&self) -> &PsiOptimizationSelections {
        &self.selections
    }

    pub const fn execution(&self) -> &PsiOptimizationExecutionRecord {
        &self.execution
    }

    pub fn into_lowered(self) -> LoweredPsi {
        self.lowered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationStageError {
    InvalidDeadScalarRewrite(terminal_verifier::DeadScalarRewriteError),
    InvalidModule(ModuleError),
    InvalidSemantic(CodecError),
    InvalidProof(ProofCodecError),
    InvalidDebugMap(DebugMapError),
    InvalidExecutionRecord(PsiOptimizationExecutionRecordError),
    UnsupportedSelection(PsiOptimization),
}

impl std::fmt::Display for PsiOptimizationStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDeadScalarRewrite(error) => {
                write!(formatter, "invalid dead scalar rewrite: {error:?}")
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
            Self::UnsupportedSelection(optimization) => write!(
                formatter,
                "Psi optimization `{}` has no pre-Terminal implementation",
                optimization.name()
            ),
        }
    }
}

impl std::error::Error for PsiOptimizationStageError {}
