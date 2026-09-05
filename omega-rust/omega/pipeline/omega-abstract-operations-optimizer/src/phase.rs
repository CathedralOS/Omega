//! Optimizer module role: executable entrance. The complete abstract X-to-X phase.

use omega_optimization_core::PsiOptimizationSelectionProjection;
use omega_optimization_core::{OptimizationSelections, OptimizationWorkBudget};
use omega_psi_to_abstract_operations::{
    VerifiedPsiOptimizationInput, VerifiedPsiOptimizationUnitBuildError,
    build_verified_psi_optimization_unit,
};

use crate::{
    OptimizationRunError, OptimizedAbstractProjectionError, ValidatedOptimizedAbstractPlan,
    publish_optimization_run, run_psi_pipeline_for_projection,
};

#[derive(Debug)]
pub enum AbstractOptimizationError {
    UnitBuild(VerifiedPsiOptimizationUnitBuildError),
    Run(OptimizationRunError),
    Publication(OptimizedAbstractProjectionError),
}

impl std::fmt::Display for AbstractOptimizationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "abstract optimization failed: {self:?}")
    }
}

impl std::error::Error for AbstractOptimizationError {}

pub fn optimize_abstract_operations(
    input: VerifiedPsiOptimizationInput,
    selections: &OptimizationSelections,
    psi_selections: &PsiOptimizationSelectionProjection,
    budget_per_pass: OptimizationWorkBudget,
) -> Result<ValidatedOptimizedAbstractPlan, AbstractOptimizationError> {
    let verified = build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .map_err(AbstractOptimizationError::UnitBuild)?;
    let run =
        run_psi_pipeline_for_projection(verified, selections, psi_selections, budget_per_pass)
            .map_err(AbstractOptimizationError::Run)?;
    publish_optimization_run(run).map_err(AbstractOptimizationError::Publication)
}
