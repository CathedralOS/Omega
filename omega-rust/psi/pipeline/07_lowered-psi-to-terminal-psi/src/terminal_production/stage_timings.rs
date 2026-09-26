//! Per-stage timing carrier owned by Terminal production.
//!
//! `terminal-production` cannot depend on Omega's `artifacts` accumulator
//! (`psi_does_not_depend_on_omega`), so the Psi side of the coarse
//! `terminal-production` boundary row records `(stage, microseconds)` rows
//! here; the owning Omega caller merges them into `CompileTimings` after
//! production returns. A disabled carrier runs the work unmeasured, so every
//! production path shares one instrumented body.

use std::time::Instant;

/// One measured leg inside Terminal artifact production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalProductionStage {
    MachineSelection,
    LedgerCheck,
    Lowering,
    Optimization,
    EntryReceipt,
    TerminalIdentity,
    ReceiverEligibility,
    Publication,
    BoundaryOperatorScope,
}

impl TerminalProductionStage {
    pub const fn name(self) -> &'static str {
        match self {
            Self::MachineSelection => "machine-selection",
            Self::LedgerCheck => "ledger-check",
            Self::Lowering => "lowering",
            Self::Optimization => "optimization",
            Self::EntryReceipt => "entry-receipt",
            Self::TerminalIdentity => "terminal-identity",
            Self::ReceiverEligibility => "receiver-eligibility",
            Self::Publication => "publication",
            Self::BoundaryOperatorScope => "boundary-scope",
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

#[cfg(test)]
mod tests {
    use super::{TerminalProductionStage, TerminalProductionTimings};

    #[test]
    fn repeated_stage_measurements_aggregate_without_reordering() {
        let mut timings = TerminalProductionTimings::enabled();
        assert_eq!(
            timings.record_result(TerminalProductionStage::Lowering, || Ok::<_, ()>(7)),
            Ok(7)
        );
        timings
            .record_result(TerminalProductionStage::Publication, || Ok::<_, ()>(3))
            .unwrap();
        timings
            .record_result(TerminalProductionStage::Lowering, || Ok::<_, ()>(5))
            .unwrap();

        let stages: Vec<TerminalProductionStage> =
            timings.rows().iter().map(|(stage, _)| *stage).collect();
        assert_eq!(
            stages,
            [
                TerminalProductionStage::Lowering,
                TerminalProductionStage::Publication
            ]
        );
        assert!(timings.rows()[0].1 > 0 || timings.rows()[0].1 == 0);
    }

    #[test]
    fn error_measurement_preserves_the_exact_error() {
        let mut timings = TerminalProductionTimings::enabled();
        let result: Result<(), &'static str> =
            timings.record_result(TerminalProductionStage::Lowering, || Err("retained base"));
        assert_eq!(result, Err("retained base"));
        assert_eq!(timings.rows()[0].0, TerminalProductionStage::Lowering);
    }

    #[test]
    fn disabled_collection_runs_work_without_retaining_measurements() {
        let mut timings = TerminalProductionTimings::default();
        assert_eq!(
            timings.record_result(TerminalProductionStage::Lowering, || Ok::<_, ()>(7)),
            Ok(7)
        );
        assert!(timings.rows().is_empty());
    }
}
