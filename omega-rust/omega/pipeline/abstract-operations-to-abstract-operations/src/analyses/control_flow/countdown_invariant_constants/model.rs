//! Optimizer module role: carrier leaf. Revision-bound countdown invariant constants.

use super::*;

/// Closed semantic role of an input-free integer constant retained by the
/// exact unsigned-countdown ranking relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CountdownInvariantConstantRole {
    PositiveGuardZero,
    BackedgeDecrementOne,
}

/// One exact source node that is invariant across every iteration of its
/// authenticated countdown component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantIntegerConstant {
    pub role: CountdownInvariantConstantRole,
    pub location: NodeLocation,
    pub psi_operation: OperationId,
    pub result: ValueId,
    pub scalar_type: IntegerType,
    pub value: IntegerValue,
    pub definition: ValueDefinition,
    pub provenance: Vec<PsiProvenance>,
    pub fuel: Vec<FuelSettlement>,
    pub effect: EffectLink,
}

/// Exact invariant rows for one independently authenticated counted loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedCountdownInvariantConstants {
    pub counted_loop: UnsignedCountdownLoopSummary,
    /// The only destination a future hoist may consider. This analysis does
    /// not authorize insertion there or mutation of the component.
    pub prospective_preheader: BlockId,
    pub constants: Vec<CountdownInvariantIntegerConstant>,
}

/// Replayable analysis facts without cyclic-rewrite or execution authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountdownInvariantConstantAnalysisSnapshot {
    pub revision: OptimizationUnitIdentity,
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub loops: Vec<UnsignedCountdownInvariantConstants>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCountdownInvariantConstantAnalysis {
    snapshot: CountdownInvariantConstantAnalysisSnapshot,
}

impl ValidatedCountdownInvariantConstantAnalysis {
    pub(super) const fn new(snapshot: CountdownInvariantConstantAnalysisSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn loops(&self) -> &[UnsignedCountdownInvariantConstants] {
        &self.snapshot.loops
    }

    pub const fn snapshot(&self) -> &CountdownInvariantConstantAnalysisSnapshot {
        &self.snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountdownInvariantConstantAnalysisError {
    CountedLoop(CountedLoopAnalysisError),
    StaleUnitIdentity {
        stored: OptimizationUnitIdentity,
        recomputed: OptimizationUnitIdentity,
    },
    TerminalIdentityMismatch,
    CountedLoopRevisionMismatch,
    ComponentRosterMismatch,
    UnsupportedInvariantConstant {
        machine: MachineId,
        operation: OperationId,
    },
    SnapshotMismatch,
}

impl std::fmt::Display for CountdownInvariantConstantAnalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "countdown invariant-constant analysis failure: {self:?}"
        )
    }
}

impl std::error::Error for CountdownInvariantConstantAnalysisError {}
