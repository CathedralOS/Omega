//! Product reporting adapter for Psi-owned Terminal production.
//!
//! Psi owns the single stage 05-07 sequence. This adapter supplies the product
//! request's timing recorder. It never lowers, verifies, selects targets or
//! executes guests.

use checked_trees::CheckedTrees;
use psi_terminal_production::TerminalMachineSelection;

use crate::custody::{ProducedTerminalArtifact, TerminalProductionCustody};
use crate::stage_timings::ProductProductionObserver;

pub use psi_terminal_production::{
    ProgramEntryTerminalReceiptError, TerminalArtifactProductionError,
};

/// Checked source and exact machine selection for product compilation.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub machine: TerminalMachineSelection<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    pub fn new(checked: &'a CheckedTrees, machine: TerminalMachineSelection<'a>) -> Self {
        Self {
            checked,
            machine,
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Publish through the same Psi operation compile-time jobs can consume,
    /// retaining the source custody and product report requested by this caller.
    pub fn produce(
        self,
        custody: TerminalProductionCustody<'_>,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let TerminalProductionCustody {
            entry_identity,
            retain_unoptimized,
            timings,
        } = custody;
        let produced = psi_terminal_production::TerminalProductionRequest {
            checked: self.checked,
            machine: self.machine,
            optimization_selections: self.optimization_selections,
        }
        .produce(
            psi_terminal_production::TerminalProductionCustody {
                entry_identity,
                retain_unoptimized,
            },
            &mut ProductProductionObserver { timings },
        )?;
        Ok(ProducedTerminalArtifact {
            production: produced,
        })
    }
}
