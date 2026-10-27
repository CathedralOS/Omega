//! Optimizer module role: executable entrance. Exact countdown constant-placement coordination.

use abstract_operations::AbstractOperation as O;
use optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite, ValueUse,
};

use crate::validation::ValidatedOptimizerCycleComponents;
use optimization_unit::{
    CycleComponentEdge, CycleComponentId, OptimizerUnsignedCountdownRankingCertificate,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

use super::{
    AnalysisRoot, AnalysisRootError, CountdownInvariantConstantAnalysisError,
    CountdownInvariantConstantRole, CountdownInvariantIntegerConstant, CountedLoopAnalysisError,
    UnsignedCountdownLoopSummary, ValidatedCountdownInvariantConstantAnalysis,
    ValidatedCountedLoopAnalysis, validate_analysis_root, validate_moved_constant_locations,
};

mod compute;
mod replay;
mod validate;

pub(crate) fn analyze_countdown_invariant_constant_placement(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
) -> Result<
    ValidatedCountdownInvariantConstantPlacementAnalysis,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    let candidate = compute::propose(unit, custody, counted, invariants)?;
    validate::accept(unit, custody, counted, invariants, &candidate)
}

/// The exact insertion coordinate immediately before the unique preheader jump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantDestination {
    pub before: NodeLocation,
    pub entry_edge: CycleComponentEdge,
}

/// The exact certificate operation that consumes one invariant constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantConsumer {
    pub location: NodeLocation,
    pub psi_operation: OperationId,
    pub value_use: ValueUse,
}

/// One analysis-only placement fact. This is not a rewrite plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantPlacement {
    pub constant: CountdownInvariantIntegerConstant,
    pub destination: CountdownInvariantConstantDestination,
    pub consumer: CountdownInvariantConstantConsumer,
}

/// Exact zero/one placement facts keyed by their authenticated component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsignedCountdownInvariantConstantPlacements {
    pub component: CycleComponentId,
    pub counted_loop: UnsignedCountdownLoopSummary,
    pub placements: Vec<CountdownInvariantConstantPlacement>,
}

/// Replayable placement facts without mutation or execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CountdownInvariantConstantPlacementAnalysisSnapshot {
    pub root: AnalysisRoot,
    pub loops: Vec<UnsignedCountdownInvariantConstantPlacements>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedCountdownInvariantConstantPlacementAnalysis {
    snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot,
}

impl ValidatedCountdownInvariantConstantPlacementAnalysis {
    const fn new(snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot) -> Self {
        Self { snapshot }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountdownInvariantConstantPlacementAnalysisError {
    CountedLoop(CountedLoopAnalysisError),
    InvariantConstant(CountdownInvariantConstantAnalysisError),
    Root(AnalysisRootError),
    ComponentRosterMismatch,
    UnsupportedPlacement {
        machine: MachineId,
        operation: OperationId,
    },
    SnapshotMismatch,
}

failed_display!(
    CountdownInvariantConstantPlacementAnalysisError,
    "countdown invariant-constant placement analysis failure"
);

impl std::error::Error for CountdownInvariantConstantPlacementAnalysisError {}
