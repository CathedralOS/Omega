//! Optimizer module role: executable entrance. Exact countdown invariant-constant coordination.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{
    EffectLink, FuelSettlement, NodeLocation, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, ValueDefinition, ValueDefinitionSite, recompute_psi_optimization_unit_identity,
};
use optimization_validation::{
    OptimizerUnsignedCountdownRankingCertificate, ValidatedOptimizerCycleComponents,
};
use semantic_vocabulary::{
    BlockId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

use super::{CountedLoopAnalysisError, UnsignedCountdownLoopSummary, ValidatedCountedLoopAnalysis};

mod compute;
mod model;
mod replay;
mod validate;

pub use model::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantRole, CountdownInvariantIntegerConstant,
    UnsignedCountdownInvariantConstants, ValidatedCountdownInvariantConstantAnalysis,
};

pub(crate) fn analyze_countdown_invariant_constants(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError> {
    let candidate = compute::propose(unit, custody, counted)?;
    validate::accept(unit, custody, counted, &candidate)
}

pub(crate) fn validate_countdown_invariant_constant_analysis(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    candidate: &CountdownInvariantConstantAnalysisSnapshot,
) -> Result<ValidatedCountdownInvariantConstantAnalysis, CountdownInvariantConstantAnalysisError> {
    validate::accept(unit, custody, counted, candidate)
}
