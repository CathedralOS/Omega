//! Optional observations around the one production sequence.
//!
//! This interface keeps host report storage outside Psi. The observer runs the
//! supplied operation exactly once and returns its result unchanged; it may
//! record duration, but it must not supply stage judgments or retry execution.

/// One measured leg inside Terminal artifact production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalProductionStage {
    MachineSelection,
    Lowering,
    Optimization,
    EntryReceipt,
    Publication,
    BoundaryOperatorScope,
}

impl TerminalProductionStage {
    /// Stable name, input and output labels for this production leg.
    pub const fn observation_labels(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::MachineSelection => (
                "terminal-production/machine-selection",
                "CheckedTrees",
                "TerminalMachine",
            ),
            Self::Lowering => ("terminal-production/lowering", "CheckedTrees", "LoweredPsi"),
            Self::Optimization => (
                "terminal-production/optimization",
                "LoweredPsi",
                "OptimizedLoweredPsi",
            ),
            Self::EntryReceipt => (
                "terminal-production/entry-receipt",
                "OptimizedLoweredPsi",
                "ProgramEntryReceipt",
            ),
            Self::Publication => (
                "terminal-production/publication",
                "OptimizedLoweredPsi",
                "CanonicalTerminalArtifact",
            ),
            Self::BoundaryOperatorScope => (
                "terminal-production/boundary-scope",
                "LoweredPsi+CanonicalTerminalArtifact",
                "BoundaryOperatorScope",
            ),
        }
    }
}

/// Caller-owned observation of one production leg.
pub trait TerminalProductionObserver {
    fn record_result<Value, Error>(
        &mut self,
        stage: TerminalProductionStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error>;
}

/// Execute the same production operation without report instrumentation.
pub struct UnmeasuredProduction;

impl TerminalProductionObserver for UnmeasuredProduction {
    fn record_result<Value, Error>(
        &mut self,
        _stage: TerminalProductionStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        operation()
    }
}
