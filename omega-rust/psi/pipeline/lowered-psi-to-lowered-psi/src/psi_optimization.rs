//! Optimizer module role: executable entrance. The target-neutral pass
//! driver and the carriers every pass respects.

mod model;
pub(crate) mod ranked;
pub(crate) mod retained;
mod validation;

pub use model::{PsiOptimizationStageError, PsiOptimizationStageResult};

use crate::{
    control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering,
    proof_check_elision, sparse_conditional_constant_propagation,
};
use lowered_psi::LoweredPsi;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_codec::PsiOptimizationExecutionRecord;
use validation::validate_carrier;

/// Execute the selected target-neutral optimization phase over the complete
/// unsealed Psi product.
///
/// The empty selection deliberately validates both sides of the identity
/// transformation. Selected passes execute in canonical order; the catalog is
/// exhaustive, so a future selection added without an implementation fails to
/// compile rather than being recorded as an executed identity.
pub fn run_psi_optimization(
    mut lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
) -> Result<PsiOptimizationStageResult, PsiOptimizationStageError> {
    let (input_semantic, input_proof) = validate_carrier(&lowered)?;
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
            PsiOptimization::ProofCheckElision => {
                lowered = proof_check_elision::elide(lowered)?;
            }
        }
    }

    let (output_semantic, output_proof) = validate_carrier(&lowered)?;
    let execution = PsiOptimizationExecutionRecord::new(
        selections.clone(),
        input_semantic,
        input_proof,
        output_semantic,
        output_proof,
    )
    .map_err(PsiOptimizationStageError::InvalidExecutionRecord)?;
    Ok(PsiOptimizationStageResult::new(
        lowered, selections, execution,
    ))
}
