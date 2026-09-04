//! Pre-Terminal optimization stage carriers and errors.

use psi_optimization::{
    PsiOptimization, PsiOptimizationSelectionIdentity, PsiOptimizationSelections,
};
use psi_terminal::TerminalPsiIdentity;
use psi_terminal_codec::{CodecError, DebugMapError, ProofBundleFingerprint, ProofCodecError};
use psi_terminal_verifier::ModuleError;

use crate::LoweredTerminalPsi;

/// Strong identity of one selected pre-Terminal optimization execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PsiOptimizationExecutionIdentity(pub(super) [u8; 32]);

impl PsiOptimizationExecutionIdentity {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical identities before and after one pre-Terminal optimization stage.
///
/// The complete [`LoweredTerminalPsi`] remains in the stage result. This record
/// binds the selected pass set to the semantic and proof products without
/// pretending that source-only sidecars are part of canonical Terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsiOptimizationExecutionRecord {
    selection: PsiOptimizationSelectionIdentity,
    input_semantic: TerminalPsiIdentity,
    input_proof: ProofBundleFingerprint,
    output_semantic: TerminalPsiIdentity,
    output_proof: ProofBundleFingerprint,
    identity: PsiOptimizationExecutionIdentity,
}

impl PsiOptimizationExecutionRecord {
    pub(super) const fn new(
        selection: PsiOptimizationSelectionIdentity,
        input_semantic: TerminalPsiIdentity,
        input_proof: ProofBundleFingerprint,
        output_semantic: TerminalPsiIdentity,
        output_proof: ProofBundleFingerprint,
        identity: PsiOptimizationExecutionIdentity,
    ) -> Self {
        Self {
            selection,
            input_semantic,
            input_proof,
            output_semantic,
            output_proof,
            identity,
        }
    }

    pub const fn selection(&self) -> PsiOptimizationSelectionIdentity {
        self.selection
    }

    pub const fn input_semantic(&self) -> TerminalPsiIdentity {
        self.input_semantic
    }

    pub const fn input_proof(&self) -> ProofBundleFingerprint {
        self.input_proof
    }

    pub const fn output_semantic(&self) -> TerminalPsiIdentity {
        self.output_semantic
    }

    pub const fn output_proof(&self) -> ProofBundleFingerprint {
        self.output_proof
    }

    pub const fn identity(&self) -> PsiOptimizationExecutionIdentity {
        self.identity
    }
}

/// Validated output of the selected target-neutral Psi optimization phase.
///
/// Terminal publication accepts this type rather than an unvalidated lowering
/// result. Empty selection is an executed identity transformation. A selected
/// pass has no route until its rewrite and independent validator operate on
/// this complete carrier, including proof, debug, and source-custody sidecars.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "Terminal publication requires the validated Psi optimization result"]
pub struct PsiOptimizationStageResult {
    lowered: LoweredTerminalPsi,
    selections: PsiOptimizationSelections,
    execution: PsiOptimizationExecutionRecord,
}

impl PsiOptimizationStageResult {
    pub(super) const fn new(
        lowered: LoweredTerminalPsi,
        selections: PsiOptimizationSelections,
        execution: PsiOptimizationExecutionRecord,
    ) -> Self {
        Self {
            lowered,
            selections,
            execution,
        }
    }

    pub const fn lowered(&self) -> &LoweredTerminalPsi {
        &self.lowered
    }

    pub const fn selections(&self) -> &PsiOptimizationSelections {
        &self.selections
    }

    pub const fn execution(&self) -> PsiOptimizationExecutionRecord {
        self.execution
    }

    pub(crate) fn into_lowered(self) -> LoweredTerminalPsi {
        self.lowered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PsiOptimizationStageError {
    InvalidModule(ModuleError),
    InvalidSemantic(CodecError),
    InvalidProof(ProofCodecError),
    InvalidDebugMap(DebugMapError),
    UnsupportedSelection(PsiOptimization),
}

impl std::fmt::Display for PsiOptimizationStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidModule(error) => write!(formatter, "invalid optimization input: {error}"),
            Self::InvalidSemantic(error) => {
                write!(formatter, "invalid optimization semantics: {error}")
            }
            Self::InvalidProof(error) => write!(formatter, "invalid optimization proof: {error}"),
            Self::InvalidDebugMap(error) => {
                write!(formatter, "invalid optimization debug map: {error}")
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
