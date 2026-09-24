//! Optimizer module role: executable entrance. Validated countdown-loop analysis coordination.

use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationUnitIdentity;
use optimization_unit::{
    PsiOptimizationFunction, PsiOptimizationUnit, recompute_psi_optimization_unit_identity,
};

use crate::validation::ValidatedOptimizerCycleComponents;
use optimization_unit::{
    CycleComponentEdge, OptimizerCycleComponent, OptimizerUnsignedCountdownRankingCertificate,
};
use semantic_vocabulary::{IntegerType, MachineId, ScalarType, ValueId};

use super::LoopRegion;

mod compute;
mod region;
mod replay;
mod validate;

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
    /// The component's reducible region projected from validated Terminal-SCC
    /// custody: the certified header is the component's unique entry target.
    pub region: LoopRegion,
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
    pub terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub loops: Vec<UnsignedCountdownLoopSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCountedLoopAnalysis {
    snapshot: CountedLoopAnalysisSnapshot,
}

impl ValidatedCountedLoopAnalysis {
    const fn new(snapshot: CountedLoopAnalysisSnapshot) -> Self {
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
