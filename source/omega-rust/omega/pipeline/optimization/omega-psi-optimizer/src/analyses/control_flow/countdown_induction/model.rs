//! Optimizer module role: carrier leaf. Revision-bound exact countdown-loop summaries.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactUnsignedTripCount {
    /// The value entering the header before the first guard evaluation.
    pub initial_value: ValueId,
    pub scalar_type: IntegerType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedCountdownLoopSummary {
    /// Complete ranking evidence is the semantic key, not a guessed header.
    pub certificate: OptimizerUnsignedCountdownRankingCertificate,
    pub members: Vec<BlockId>,
    pub preheader_edge: CycleComponentEdge,
    pub exit_edge: CycleComponentEdge,
    /// For the exact `rank > 0; rank - 1` relation, the entering unsigned
    /// value is also the symbolic exact trip count.
    pub trip_count: ExactUnsignedTripCount,
}

/// Replayable counted-loop facts without authority to transform or execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedLoopAnalysisSnapshot {
    pub revision: OptimizationUnitIdentity,
    pub terminal_psi: psi_terminal::TerminalPsiIdentity,
    pub loops: Vec<UnsignedCountdownLoopSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCountedLoopAnalysis {
    snapshot: CountedLoopAnalysisSnapshot,
}

impl ValidatedCountedLoopAnalysis {
    pub(super) const fn new(snapshot: CountedLoopAnalysisSnapshot) -> Self {
        Self { snapshot }
    }

    pub fn loops(&self) -> &[UnsignedCountdownLoopSummary] {
        &self.snapshot.loops
    }

    pub const fn snapshot(&self) -> &CountedLoopAnalysisSnapshot {
        &self.snapshot
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountedLoopAnalysisError {
    StaleUnitIdentity {
        stored: OptimizationUnitIdentity,
        recomputed: OptimizationUnitIdentity,
    },
    TerminalIdentityMismatch,
    CertificateComponentRosterMismatch,
    UnsupportedCountdownShape {
        machine: MachineId,
    },
    SnapshotMismatch,
}

impl std::fmt::Display for CountedLoopAnalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "counted-loop analysis failure: {self:?}")
    }
}

impl std::error::Error for CountedLoopAnalysisError {}
