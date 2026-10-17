//! Execute selected target-neutral passes and record their publication identities.

use crate::optimization_error::PsiOptimizationStageError;

use crate::{
    control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering,
    sparse_conditional_constant_propagation,
};
use lowered_psi::LoweredPsi;
use optimization::{PRETERMINAL_PSI_PASS_CATALOG, PsiOptimization, PsiOptimizationSelections};
use terminal_codec::PsiOptimizationExecutionRecord;
use terminal_codec::terminal_psi_identity;
use terminal_psi::TerminalPsiIdentity;

/// Execute the selected target-neutral optimization phase over the complete
/// unsealed Psi product.
///
/// Input and output fingerprints identify the transformation; they do not
/// revalidate the producer's module. Selected passes execute in canonical order; the catalog is
/// exhaustive, so a future selection added without an implementation fails to
/// compile rather than being silently recorded as an executed identity — every
/// member's preterminal semantics must be stated here explicitly.
/// `StateSpecialization` and `RepresentationSpecialization` rewrite the
/// abstract-operations unit produced only after this stage, so they execute
/// in the post-terminal stage and are left out of this stage's execution
/// record.
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
    let executed = PsiOptimizationSelections::new(
        selections
            .as_slice()
            .iter()
            .copied()
            .filter(|selected| PRETERMINAL_PSI_PASS_CATALOG.contains(selected)),
    )
    .expect("a filtered selection subset stays duplicate-free");
    let execution = PsiOptimizationExecutionRecord::new(executed, input_semantic, output_semantic)
        .map_err(PsiOptimizationStageError::InvalidExecutionRecord)?;
    Ok(PsiOptimizationStageResult::new(
        lowered, selections, execution,
    ))
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
/// together. Empty selection is an executed identity transformation. Passes
/// preserve the proof, debug and source-custody sidecars; independent validators
/// exercise their invariants in tests rather than repeating production work.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "Terminal publication requires the Psi optimization result"]
pub struct PsiOptimizationStageResult {
    lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
    execution: PsiOptimizationExecutionRecord,
}

impl PsiOptimizationStageResult {
    const fn new(
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

    pub fn into_parts(self) -> (LoweredPsi, PsiOptimizationExecutionRecord) {
        (self.lowered, self.execution)
    }
}
