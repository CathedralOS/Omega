//! Sequence the selected Psi lowering, optimization and publication owners.

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{
    LoweringError, TerminalMachineSelection, lower_machine, lowered_machine_provenance,
    select_terminal_machine,
};
use diagnostics::Diagnostic;
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};

use crate::custody::{ProducedTerminalArtifact, TerminalProductionCustody};
use crate::observation::{TerminalProductionObserver, TerminalProductionStage};

pub use lowered_psi_to_terminal_psi::ProgramEntryTerminalReceiptError;
use lowered_psi_to_terminal_psi::{prepare_program_entry_receipt, rejoin_program_entry_receipt};

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
pub struct TerminalProductionRequest<'a> {
    pub checked: &'a CheckedTrees,
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub machine: TerminalMachineSelection<'a>,
    pub optimization_selections: optimization::PsiOptimizationSelections,
}

impl<'a> TerminalProductionRequest<'a> {
    /// Select the identity optimization phase by default.
    pub fn new(
        checked: &'a CheckedTrees,
        machine: TerminalMachineSelection<'a>,
        exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    ) -> Self {
        Self {
            checked,
            exact_toolchain_sources,
            machine,
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Produce the canonical Terminal artifact beside the custody the caller
    /// asked to retain.
    ///
    /// Every product shares one instrumented body: checked-input lowering,
    /// selected optimization, the `ProgramEntry` receipt legs when an entry
    /// identity is supplied, and canonical publication including boundary
    /// demands and checked body candidates. The source-signature digest remains
    /// opaque; later Omega settlement must compare it with the retained source
    /// signature.
    pub fn produce(
        self,
        custody: TerminalProductionCustody<'_>,
        observer: &mut impl TerminalProductionObserver,
    ) -> Result<ProducedTerminalArtifact, TerminalArtifactProductionError> {
        let TerminalProductionCustody {
            entry_identity,
            admission,
        } = custody;
        let checked = self.checked;
        // The receipt names the selected checked machine, so the selection is
        // rejoined for receipts and diagnostic attribution before lowering.
        let entry_selection = match (entry_identity, admission.is_some()) {
            (Some(_), _) | (_, true) => Some(
                observer
                    .record_result(TerminalProductionStage::MachineSelection, || {
                        select_terminal_machine(checked, self.machine)
                    })
                    .map_err(TerminalArtifactProductionError::Lowering)?,
            ),
            _ => None,
        };
        let lowered = observer
            .record_result(TerminalProductionStage::Lowering, || {
                lower_machine(self.checked, self.machine, self.exact_toolchain_sources)
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        if let (Some(admission), Some(selection)) = (admission, entry_selection) {
            let provenance = lowered_machine_provenance(checked, &lowered, selection.machine);
            admission
                .admit(&lowered.semantic_module, &provenance)
                .map_err(TerminalArtifactProductionError::Admission)?;
        }
        let optimized = observer
            .record_result(TerminalProductionStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)?;
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
        let artifact = observer
            .record_result(TerminalProductionStage::Publication, || {
                lowered_psi_to_terminal_psi::finalize_terminal_artifact(optimized)
            })
            .map_err(TerminalArtifactProductionError::Artifact)?;
        let receipt = match entry_receipt {
            Some(receipt) => {
                rejoin_program_entry_receipt(&receipt, &artifact)
                    .map_err(TerminalArtifactProductionError::EntryReceipt)?;
                Some(receipt)
            }
            None => None,
        };
        Ok(ProducedTerminalArtifact { artifact, receipt })
    }
}

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    Admission(Vec<Diagnostic>),
    Lowering(LoweringError),
    Optimization(PsiOptimizationStageError),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    EntryReceipt(ProgramEntryTerminalReceiptError),
}

impl TerminalArtifactProductionError {
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match self {
            Self::Admission(diagnostics) => diagnostics.clone(),
            _ => vec![self.diagnostic()],
        }
    }

    /// The refusal as the diagnostic compilation reports. A body this
    /// compiler cannot lower yet reports its plan gap: the construct at its
    /// source span, as an `Incomplete` coverage provision.
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::Lowering(error) => error.plan_gap().cloned(),
            _ => None,
        }
        .unwrap_or_else(|| {
            Diagnostic::error(format!("terminal-artifact production failed: {self}"))
        })
    }
}

impl std::fmt::Display for TerminalArtifactProductionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TerminalArtifactProductionError {}
