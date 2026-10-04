//! Per-stage timing carrier owned by Terminal production.
//!
//! The product driver's narrow production sequence records its completed
//! `(stage, microseconds)` rows here. The request-level caller merges them
//! into `CompileTimings`; Psi stages need no reporting dependency. A disabled
//! carrier runs the same work unmeasured.

use std::time::Instant;

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
    /// Stable name, input and output labels for this production leg. Consumers
    /// merge measured rows without maintaining their own stage taxonomy.
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

/// Completed production legs, in run order. Repeated visits to one stage
/// aggregate onto its first row so the ladder keeps call order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalProductionTimings {
    enabled: bool,
    rows: Vec<(TerminalProductionStage, u128)>,
}

impl TerminalProductionTimings {
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            rows: Vec::new(),
        }
    }

    /// Mirror the request's collection state: enabled measures each leg,
    /// disabled runs the same instrumented body without clock reads or rows.
    pub fn enabled_if(enabled: bool) -> Self {
        if enabled {
            Self::enabled()
        } else {
            Self::default()
        }
    }

    pub fn rows(&self) -> &[(TerminalProductionStage, u128)] {
        &self.rows
    }

    pub fn record_result<T, E>(
        &mut self,
        stage: TerminalProductionStage,
        work: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        if !self.enabled {
            return work();
        }
        let started = Instant::now();
        let result = work();
        let microseconds = started.elapsed().as_micros();
        if let Some(row) = self.rows.iter_mut().find(|(name, _)| *name == stage) {
            row.1 = row.1.saturating_add(microseconds);
        } else {
            self.rows.push((stage, microseconds));
        }
        result
    }
}
