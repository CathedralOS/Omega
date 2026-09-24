//! Optimizer module role: executable entrance. Exact countdown constant-placement coordination.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{
    NodeLocation, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    ValueDefinitionSite, ValueUse, recompute_psi_optimization_unit_identity,
};

use crate::validation::ValidatedOptimizerCycleComponents;
use optimization_unit::{
    CycleComponentEdge, CycleComponentId, OptimizerUnsignedCountdownRankingCertificate,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};

use super::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantRole,
    CountdownInvariantIntegerConstant, CountedLoopAnalysisError, UnsignedCountdownLoopSummary,
    ValidatedCountdownInvariantConstantAnalysis, ValidatedCountedLoopAnalysis,
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

pub(crate) fn validate_countdown_invariant_constant_placement_analysis(
    unit: &PsiOptimizationUnit,
    custody: &ValidatedOptimizerCycleComponents,
    counted: &ValidatedCountedLoopAnalysis,
    invariants: &ValidatedCountdownInvariantConstantAnalysis,
    candidate: &CountdownInvariantConstantPlacementAnalysisSnapshot,
) -> Result<
    ValidatedCountdownInvariantConstantPlacementAnalysis,
    CountdownInvariantConstantPlacementAnalysisError,
> {
    validate::accept(unit, custody, counted, invariants, candidate)
}

/// The exact insertion coordinate immediately before the unique preheader jump.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantDestination {
    pub before: NodeLocation,
    pub entry_edge: CycleComponentEdge,
}

/// The exact certificate operation that consumes one invariant constant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantConsumer {
    pub location: NodeLocation,
    pub psi_operation: OperationId,
    pub value_use: ValueUse,
}

/// One analysis-only placement fact. This is not a rewrite plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantPlacement {
    pub constant: CountdownInvariantIntegerConstant,
    pub destination: CountdownInvariantConstantDestination,
    pub consumer: CountdownInvariantConstantConsumer,
}

/// Exact zero/one placement facts keyed by their authenticated component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedCountdownInvariantConstantPlacements {
    pub component: CycleComponentId,
    pub counted_loop: UnsignedCountdownLoopSummary,
    pub placements: Vec<CountdownInvariantConstantPlacement>,
}

/// Replayable placement facts without mutation or execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantPlacementAnalysisSnapshot {
    pub revision: OptimizationUnitIdentity,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub loops: Vec<UnsignedCountdownInvariantConstantPlacements>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCountdownInvariantConstantPlacementAnalysis {
    snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot,
}

impl ValidatedCountdownInvariantConstantPlacementAnalysis {
    const fn new(snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn loops(&self) -> &[UnsignedCountdownInvariantConstantPlacements] {
        &self.snapshot.loops
    }

    pub const fn snapshot(&self) -> &CountdownInvariantConstantPlacementAnalysisSnapshot {
        &self.snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountdownInvariantConstantPlacementAnalysisError {
    CountedLoop(CountedLoopAnalysisError),
    InvariantConstant(CountdownInvariantConstantAnalysisError),
    StaleUnitIdentity {
        stored: OptimizationUnitIdentity,
        recomputed: OptimizationUnitIdentity,
    },
    TerminalIdentityMismatch,
    AnalysisRevisionMismatch,
    CandidateRevisionMismatch {
        candidate: OptimizationUnitIdentity,
        current: OptimizationUnitIdentity,
    },
    ComponentRosterMismatch,
    UnsupportedPlacement {
        machine: MachineId,
        operation: OperationId,
    },
    SnapshotMismatch,
}

impl std::fmt::Display for CountdownInvariantConstantPlacementAnalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "countdown invariant-constant placement analysis failure: {self:?}"
        )
    }
}

impl std::error::Error for CountdownInvariantConstantPlacementAnalysisError {}
