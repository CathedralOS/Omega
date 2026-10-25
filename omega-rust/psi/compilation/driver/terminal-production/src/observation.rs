//! Optional observations around the one production sequence.
//!
//! This interface keeps host report storage outside Psi. The observer runs the
//! supplied operation exactly once and returns its result unchanged; it may
//! record duration, but it must not supply stage judgments or retry execution.

/// One measured leg inside source compilation or Terminal artifact production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum PsiCompilationStage {
    MachineSelection,
    Lowering,
    Optimization,
    EntryReceipt,
    Publication,
    Resolution,
    Typing,
    Lexing,
    Parsing,
    SourceParsing,
}

/// Stable name, input and output labels for each production leg, in
/// `PsiCompilationStage` declaration order.
const OBSERVATION_LABELS: [(&'static str, &'static str, &'static str); 10] = [
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
    ("Stage 03", "SyntaxTrees", "SymbolResolvedTrees"),
    ("Stage 04", "SymbolResolvedTrees", "TypedTrees"),
    ("Stage 01", "SourceFiles", "TokenStreams"),
    ("Stage 02", "TokenStreams", "SyntaxTrees"),
    ("Source ingress", "SourceFiles", "SyntaxTrees"),
];

impl PsiCompilationStage {
    /// Stable name, input and output labels for this production leg.
    pub const fn observation_labels(self) -> (&'static str, &'static str, &'static str) {
        OBSERVATION_LABELS[self as usize]
    }
}

/// Caller-owned observation of one production leg.
pub trait PsiCompilationObserver {
    fn record_result<Value, Error>(
        &mut self,
        stage: PsiCompilationStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error>;
}

/// Execute the same production operation without report instrumentation.
pub struct UnmeasuredProduction;

impl PsiCompilationObserver for UnmeasuredProduction {
    fn record_result<Value, Error>(
        &mut self,
        _stage: PsiCompilationStage,
        operation: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        operation()
    }
}
