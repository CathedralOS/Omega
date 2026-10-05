//! Product reporting adapter for Psi-owned Terminal production.
//!
//! Psi owns the single stage 05-07 sequence. This adapter supplies the product
//! request's timing recorder and maps Psi optimizer refusals to Omega report
//! vocabulary. It never lowers, verifies, selects targets or executes guests.

use checked_trees::CheckedTrees;
use lowered_psi_to_lowered_psi::{PsiRewriteRefusal, PsiRewriteRefusalReason};
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
        let withheld_rewrites = withheld_rewrites(produced.withheld_rewrites());
        Ok(ProducedTerminalArtifact {
            production: produced,
            withheld_rewrites,
        })
    }
}

/// Product reports retain the same reason codes as before the ownership move.
/// A refusal leaves the authored artifact intact; it is not a failed compile.
fn withheld_rewrites(refusals: &[PsiRewriteRefusal]) -> Vec<optimization_core::WithheldRewrite> {
    refusals
        .iter()
        .map(|refusal| {
            let reason = match refusal.reason() {
                PsiRewriteRefusalReason::ChangedProofQuestion => {
                    optimization_core::OptimizationReasonCode::ValidationFailed
                }
                PsiRewriteRefusalReason::ProposalLeftProofContextDangling => {
                    optimization_core::OptimizationReasonCode::ProofIncomplete
                }
            };
            optimization_core::WithheldRewrite::new(refusal.optimization(), reason)
        })
        .collect()
}
