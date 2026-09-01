//! Optimizer module role: executable entrance. Exact countdown invariant-constant coordination.

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_core::OptimizationUnitIdentity;
use omega_optimization_unit::{
    EffectLink, FuelSettlement, NodeLocation, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiProvenance, ValueDefinition, ValueDefinitionSite, recompute_psi_optimization_unit_identity,
};
use omega_optimization_validation::{
    CycleComponentId, OptimizerUnsignedCountdownRankingCertificate,
    ValidatedOptimizerCycleComponents,
};
use psi_core::{BlockId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId};

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
