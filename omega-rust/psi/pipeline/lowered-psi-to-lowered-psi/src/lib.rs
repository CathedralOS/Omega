#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance.
//!
//! Target-neutral optimization between checked lowering and Terminal
//! publication. The phase consumes the complete unsealed Psi product and
//! returns the only carrier accepted by canonical Terminal publication.

mod dead_scalar_elimination;
mod model;
mod validation;

use optimization::{PsiOptimization, PsiOptimizationSelections};

use lowered_psi::LoweredPsi;
pub use model::{PsiOptimizationStageError, PsiOptimizationStageResult};
pub use terminal_codec::{PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord};
use validation::validate_carrier;

/// Execute the selected target-neutral optimization phase over the complete
/// unsealed Psi product.
///
/// The empty selection deliberately validates both sides of the identity
/// transformation. Selected passes execute in canonical order; unported passes
/// fail closed instead of being recorded as executed identities.
pub fn run_psi_optimization(
    mut lowered: LoweredPsi,
    selections: PsiOptimizationSelections,
) -> Result<PsiOptimizationStageResult, PsiOptimizationStageError> {
    let (input_semantic, input_proof) = validate_carrier(&lowered)?;
    for selected in selections.as_slice() {
        match selected {
            PsiOptimization::DeadPureScalarElimination => {
                lowered = dead_scalar_elimination::eliminate(lowered)?;
            }
            unsupported => {
                return Err(PsiOptimizationStageError::UnsupportedSelection(
                    *unsupported,
                ));
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
