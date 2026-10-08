//! Optional observations around the one production sequence.
//!
//! This interface keeps host report storage outside Psi. The observer runs the
//! supplied operation exactly once and returns its result unchanged; it may
//! record duration, but it must not supply stage judgments or retry execution.

/// One measured leg inside Terminal artifact production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TerminalProductionStage {
    MachineSelection,
    Lowering,
    Optimization,
    EntryReceipt,
    Publication,
}

/// Stable name, input and output labels for each production leg, in
/// `TerminalProductionStage` declaration order.
const OBSERVATION_LABELS: [(&'static str, &'static str, &'static str); 5] = [
    (
        "terminal-production/machine-selection",
        "CheckedTrees",
        "TerminalMachine",
    ),
    ("terminal-production/lowering", "CheckedTrees", "LoweredPsi"),
    (
        "terminal-production/optimization",
        "LoweredPsi",
        "OptimizedLoweredPsi",
    ),
    (
        "terminal-production/entry-receipt",
        "OptimizedLoweredPsi",
        "ProgramEntryReceipt",
    ),
    (
        "terminal-production/publication",
        "OptimizedLoweredPsi",
        "CanonicalTerminalArtifact",
    ),
];

impl TerminalProductionStage {
    /// Stable name, input and output labels for this production leg.
    pub const fn observation_labels(self) -> (&'static str, &'static str, &'static str) {
        OBSERVATION_LABELS[self as usize]
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
