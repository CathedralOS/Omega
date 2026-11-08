/*
Target-neutral optimization stage coordinator (`run_psi_optimization` and
`PsiOptimizationStageResult`) and semantic carrier identity recording.
*/

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

pub fn run_psi_optimization(
    mut lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
) -> Result<PsiOptimizationStageResult, PsiOptimizationStageError> {
    let input_semantic = carrier_identity(&lowered)?;
    let mut rewritten = false;
    for selected in selections.as_slice() {
        rewritten |= !matches!(
            selected,
            PsiOptimization::ProofCheckElision
                | PsiOptimization::StateSpecialization
                | PsiOptimization::RepresentationSpecialization
                | PsiOptimization::LoopInvariantScalarMotion
        );
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

    let output_semantic = if rewritten {
        carrier_identity(&lowered)?
    } else {
        input_semantic
    };
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
