//! Optimizer module role: validation leaf. Independent Terminal/current component identity replay.

use super::{
    OptimizationUnitValidationError, OptimizerCycleComponentSnapshot, PsiOptimizationUnit,
};
pub(super) fn rederive_exact_components(
    module: &terminal_psi::TerminalModule,
    unit: &PsiOptimizationUnit,
) -> Result<OptimizerCycleComponentSnapshot, OptimizationUnitValidationError> {
    let terminal_psi = terminal_codec::terminal_psi_identity(module)
        .map_err(OptimizationUnitValidationError::ContextIdentity)?;
    super::ordinary::rederive_components(module, unit, terminal_psi)
}
