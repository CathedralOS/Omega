//! Optimizer module role: executable entrance. Validated countdown-loop analysis coordination.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationUnitIdentity;
use omega_optimization_unit::{
    PsiOptimizationFunction, PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
};
use omega_optimization_validation::{
    CycleComponentEdge, OptimizerCycleComponent, OptimizerUnsignedCountdownRankingCertificate,
    ValidatedOptimizerCycleComponents,
};
use psi_core::{BlockId, IntegerType, MachineId, ScalarType, ValueId};

mod compute;
mod model;
mod replay;
mod validate;

pub use model::{
    CountedLoopAnalysisError, CountedLoopAnalysisSnapshot, ExactUnsignedTripCount,
    UnsignedCountdownLoopSummary, ValidatedCountedLoopAnalysis,
};

pub(crate) fn analyze_counted_loops(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
    let candidate = compute::propose(unit, custody)?;
    validate::accept(unit, custody, &candidate)
}

pub(crate) fn validate_counted_loop_analysis(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    candidate: &CountedLoopAnalysisSnapshot,
) -> Result<ValidatedCountedLoopAnalysis, CountedLoopAnalysisError> {
    validate::accept(unit, custody, candidate)
}
