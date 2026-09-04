//! Optimizer module role: executable entrance.
//!
//! Target-neutral optimization between checked lowering and Terminal
//! publication. The phase consumes the complete unsealed Psi product and
//! returns the only carrier accepted by canonical Terminal publication.

mod identity;
mod model;
mod validation;

use psi_optimization::PsiOptimizationSelections;

use crate::LoweredTerminalPsi;
use identity::execution_identity;
pub use model::{
    PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord, PsiOptimizationStageError,
    PsiOptimizationStageResult,
};
use validation::validate_carrier;

/// Execute the selected target-neutral optimization phase over the complete
/// unsealed Psi product.
///
/// The empty selection deliberately validates both sides of the identity
/// transformation. Nonempty selections fail closed until their transformations
/// and validators have moved from the post-Terminal Omega optimization unit.
pub fn run_psi_optimization(
    lowered: LoweredTerminalPsi,
    selections: PsiOptimizationSelections,
) -> Result<PsiOptimizationStageResult, PsiOptimizationStageError> {
    let (input_semantic, input_proof) = validate_carrier(&lowered)?;
    if let Some(unsupported) = selections.as_slice().first().copied() {
        return Err(PsiOptimizationStageError::UnsupportedSelection(unsupported));
    }

    let (output_semantic, output_proof) = validate_carrier(&lowered)?;
    let selection = selections.identity();
    let execution = PsiOptimizationExecutionRecord::new(
        selection,
        input_semantic,
        input_proof,
        output_semantic,
        output_proof,
        execution_identity(
            selection,
            input_semantic,
            input_proof,
            output_semantic,
            output_proof,
        ),
    );
    Ok(PsiOptimizationStageResult::new(
        lowered, selections, execution,
    ))
}
