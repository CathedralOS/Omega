//! Sequence the selected Psi lowering, optimization and publication owners.

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{
    LoweringError, TerminalMachineSelection, lower_machine, select_terminal_machine,
};
use diagnostics::Diagnostic;
use lowered_psi::LoweredPsi;
use lowered_psi_to_lowered_psi::{
    PsiOptimizationStageError, PsiOptimizationStageResult, run_psi_optimization,
};
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;

use crate::custody::{ProducedTerminalArtifact, TerminalProductionCustody};
use crate::observation::{TerminalProductionObserver, TerminalProductionStage};

pub use lowered_psi_to_terminal_psi::ProgramEntryTerminalReceiptError;
use lowered_psi_to_terminal_psi::{prepare_program_entry_receipt, rejoin_program_entry_receipt};

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub machine: TerminalMachineSelection<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    /// Select the identity optimization phase by default.
    pub fn new(checked: &'a CheckedTrees, machine: TerminalMachineSelection<'a>) -> Self {
        Self {
            checked,
            machine,
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Produce the canonical Terminal artifact beside the custody the caller
    /// asked to retain.
    ///
    /// Every product shares one instrumented body: checked-input lowering,
    /// selected optimization, the `ProgramEntry` receipt legs when an entry
    /// identity is supplied, publication, and the checked boundary-operator
    /// scope. The source-signature digest remains opaque; later Omega
    /// settlement must independently compare it with the retained source
    /// signature.
    pub fn produce(
        self,
        custody: TerminalProductionCustody,
        observer: &mut impl TerminalProductionObserver,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let TerminalProductionCustody {
            entry_identity,
            retain_unoptimized,
        } = custody;
        self.produce_retained(entry_identity, retain_unoptimized, observer)
    }

    fn produce_retained(
        self,
        entry_identity: Option<[u8; 32]>,
        retain_unoptimized: bool,
        observer: &mut impl TerminalProductionObserver,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let checked = self.checked;
        // The receipt names the selected checked machine, so the selection is
        // rejoined before lowering only when an entry receipt is requested.
        let entry_selection = match entry_identity {
            Some(_) => Some(
                observer
                    .record_result(TerminalProductionStage::MachineSelection, || {
                        select_terminal_machine(checked, self.machine)
                    })
                    .map_err(TerminalArtifactProductionError::Lowering)?,
            ),
            None => None,
        };
        let (unoptimized, optimized) = self.lower_and_optimize(retain_unoptimized, observer)?;
        let entry_receipt = match (entry_identity, entry_selection) {
            (Some(source_signature_identity), Some(selection)) => Some(
                observer
                    .record_result(TerminalProductionStage::EntryReceipt, || {
                        prepare_program_entry_receipt(
                            checked,
                            selection,
                            source_signature_identity,
                            optimized.lowered(),
                        )
                    })
                    .map_err(TerminalArtifactProductionError::EntryReceipt)?,
            ),
            _ => None,
        };
        let (artifact, lowered) = observer
            .record_result(TerminalProductionStage::Publication, || {
                publish_terminal_artifact(optimized, checked)
            })?;
        let receipt = match entry_receipt {
            Some(receipt) => {
                rejoin_program_entry_receipt(&receipt, &artifact)
                    .map_err(TerminalArtifactProductionError::EntryReceipt)?;
                Some(receipt)
            }
            None => None,
        };
        let boundary_operator_scope = observer
            .record_result(TerminalProductionStage::BoundaryOperatorScope, || {
                checked_boundary_operator_scope(checked, &artifact, &lowered)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        Ok(ProducedTerminalArtifact {
            artifact,
            receipt,
            unoptimized,
            boundary_operator_scope,
            source_call_occurrences: lowered.source_call_occurrences,
            selected_ieee_float_comparison_occurrences: lowered
                .selected_ieee_float_comparison_occurrences,
            selected_integer_comparison_occurrences: lowered
                .selected_integer_comparison_occurrences,
        })
    }

    /// Lower once, retain the unoptimized module when asked, then run the
    /// selected optimization on the same lowering.
    fn lower_and_optimize(
        self,
        retain_unoptimized: bool,
        observer: &mut impl TerminalProductionObserver,
    ) -> Result<(Option<LoweredPsi>, PsiOptimizationStageResult), TerminalArtifactProductionError>
    {
        let lowered = observer
            .record_result(TerminalProductionStage::Lowering, || {
                lower_machine(self.checked, self.machine)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        let unoptimized = retain_unoptimized.then(|| lowered.clone());
        let optimized = observer
            .record_result(TerminalProductionStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)?;
        Ok((unoptimized, optimized))
    }
}

/// Publication consumes exactly the validated optimization result; retained
/// source evidence is extracted only after canonical publication succeeds.
///
fn publish_terminal_artifact(
    optimized: PsiOptimizationStageResult,
    checked: &CheckedTrees,
) -> Result<(terminal_codec::CanonicalTerminalArtifact, LoweredPsi), TerminalArtifactProductionError>
{
    let artifact = lowered_psi_to_terminal_psi::finalize_terminal_artifact(&optimized)
        .map_err(TerminalArtifactProductionError::Artifact)?;
    Ok((artifact, optimized.into_lowered()))
}

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    Lowering(LoweringError),
    Optimization(PsiOptimizationStageError),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    EntryReceipt(ProgramEntryTerminalReceiptError),
}

impl TerminalArtifactProductionError {
    /// The refusal as the diagnostic compilation reports.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.diagnostic_message())
    }

    /// Report the production verdict without making a consumer classify a
    /// lowerer's private failures.
    pub fn diagnostic_message(&self) -> String {
        if let Some(report) = self.unimplemented_report() {
            return report.to_owned();
        }
        format!("terminal-artifact production failed: {self}")
    }

    /// The `unimplemented:` report of a refusal caused by a missing
    /// compiler route rather than by the program, when this is one.
    pub fn unimplemented_report(&self) -> Option<&str> {
        match self {
            Self::Lowering(error) => error.unimplemented_report(),
            _ => None,
        }
    }
}

impl std::fmt::Display for TerminalArtifactProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactProductionError {}

fn checked_boundary_operator_scope(
    checked: &CheckedTrees,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    lowered: &LoweredPsi,
) -> Result<CheckedBoundaryOperatorApplicationScope, LoweringError> {
    lowered_psi_to_terminal_psi::checked_boundary_operator_scope(checked, artifact, lowered)
        .map_err(LoweringError::Unsupported)
}
