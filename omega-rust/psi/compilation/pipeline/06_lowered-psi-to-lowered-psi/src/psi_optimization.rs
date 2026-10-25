//! Execute selected target-neutral passes and record their publication identities.

use crate::optimization_error::PsiOptimizationStageError;

use crate::{
    control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering,
    sparse_conditional_constant_propagation,
};
use lowered_psi::LoweredPsi;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_codec::PsiOptimizationExecutionRecord;
use terminal_codec::terminal_psi_identity;
use terminal_psi::TerminalPsiIdentity;

/// Execute the selected target-neutral optimization phase over the complete
/// unsealed Psi product.
///
/// Input and output fingerprints identify the transformation; they do not
/// revalidate the producer's module. Selected passes execute in canonical
/// order; the dispatch below is exhaustive over the closed vocabulary, so a
/// member added without a pre-Terminal implementation fails to compile rather
/// than being silently recorded as an executed identity — every member's
/// pre-Terminal semantics must be stated here explicitly.
/// `ProofCheckElision`, `StateSpecialization`,
/// `RepresentationSpecialization` and `LoopInvariantScalarMotion` are
/// implemented in the post-Terminal stage only, so they run no rule here; the
/// execution record retains the exact selection regardless, because it
/// identifies the selected pass set rather than the subset that ran a rule.
pub fn run_psi_optimization(
    mut lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
) -> Result<PsiOptimizationStageResult, PsiOptimizationStageError> {
    let input_semantic = carrier_identity(&lowered)?;
    for selected in selections.as_slice() {
        match selected {
            PsiOptimization::ControlFlowCleanup => {
                lowered = control_flow_cleanup::cleanup(lowered)?;
            }
            PsiOptimization::SparseConditionalConstantPropagation => {
                lowered = sparse_conditional_constant_propagation::propagate(lowered)?;
            }
            PsiOptimization::CopyPropagation => {
                lowered = copy_propagation::propagate(lowered)?;
            }
            PsiOptimization::GlobalValueNumbering => {
                lowered = global_value_numbering::number(lowered)?;
            }
            PsiOptimization::DeadPureScalarElimination => {
                lowered = dead_scalar_elimination::eliminate(lowered)?;
            }
            PsiOptimization::ProofCheckElision
            | PsiOptimization::StateSpecialization
            | PsiOptimization::RepresentationSpecialization
            | PsiOptimization::LoopInvariantScalarMotion => {}
        }
    }

    let output_semantic = carrier_identity(&lowered)?;
    let execution =
        PsiOptimizationExecutionRecord::new(selections, input_semantic, output_semantic)
            .map_err(PsiOptimizationStageError::InvalidExecutionRecord)?;
    Ok(PsiOptimizationStageResult::new(lowered, execution))
}

fn carrier_identity(
    lowered: &LoweredPsi,
) -> Result<TerminalPsiIdentity, PsiOptimizationStageError> {
    terminal_psi_identity(&lowered.semantic_module)
        .map_err(PsiOptimizationStageError::InvalidSemantic)
}

/// Output of the selected target-neutral Psi optimization phase.
///
/// Terminal publication accepts the complete carrier and the execution record
/// together. The record owns the exact selection, so the stage result stores it
/// once. Empty selection is an executed identity transformation. Passes
/// preserve the proof, debug and source-custody sidecars; independent
/// validators exercise their invariants in tests rather than repeating
/// production work.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "Terminal publication requires the Psi optimization result"]
pub struct PsiOptimizationStageResult {
    lowered: LoweredPsi,
    execution: PsiOptimizationExecutionRecord,
}

impl PsiOptimizationStageResult {
    const fn new(lowered: LoweredPsi, execution: PsiOptimizationExecutionRecord) -> Self {
        Self { lowered, execution }
    }

    pub const fn lowered(&self) -> &LoweredPsi {
        &self.lowered
    }

    /// The selection this stage executed, read from the execution record that
    /// owns it.
    pub const fn selections(&self) -> &PsiOptimizationSelections {
        self.execution.selections()
    }

    pub const fn execution(&self) -> &PsiOptimizationExecutionRecord {
        &self.execution
    }

    pub fn into_parts(self) -> (LoweredPsi, PsiOptimizationExecutionRecord) {
        (self.lowered, self.execution)
    }
}
