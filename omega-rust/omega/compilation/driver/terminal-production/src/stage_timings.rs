//! The stage taxonomy Terminal production measures itself against.
//!
//! Production records straight into the request's `CompileTimings`. It used to
//! fill a private carrier that the request-level caller merged afterwards, but
//! that carrier duplicated `CompileTimings` exactly — the same enabled flag,
//! the same aggregate-onto-the-existing-row rule, the same `record_result`
//! closure — so only the stage names were ever production's own. They stay
//! here; the measuring does not. The Psi stages production calls still take no
//! reporting dependency: they are timed by the closure around them, not
//! instrumented themselves.

use artifacts::compile_timings::{StageMeta, TimingCategory};

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
    /// The reporting row this production leg records into.
    pub const fn stage_meta(self) -> StageMeta {
        let (name, input, output) = self.observation_labels();
        StageMeta::new(name, input, output, TimingCategory::Pipeline)
    }

    /// Stable name, input and output labels for this production leg.
    const fn observation_labels(self) -> (&'static str, &'static str, &'static str) {
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
