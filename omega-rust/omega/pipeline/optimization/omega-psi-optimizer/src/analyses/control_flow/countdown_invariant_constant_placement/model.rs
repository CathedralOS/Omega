//! Optimizer module role: carrier leaf. Revision-bound exact countdown constant placements.

use super::*;

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
    pub terminal_psi: psi_terminal::TerminalPsiIdentity,
    pub loops: Vec<UnsignedCountdownInvariantConstantPlacements>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCountdownInvariantConstantPlacementAnalysis {
    snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot,
}

impl ValidatedCountdownInvariantConstantPlacementAnalysis {
    pub(super) const fn new(snapshot: CountdownInvariantConstantPlacementAnalysisSnapshot) -> Self {
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
