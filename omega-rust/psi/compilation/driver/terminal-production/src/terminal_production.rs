//! Sequence the selected Psi lowering, optimization and publication owners.

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{
    LoweringError, TerminalMachineSelection, lower_machine, lowered_machine_provenance,
    select_terminal_machine,
};
use diagnostics::Diagnostic;
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};

use crate::custody::{ProducedTerminalProgram, TerminalProductionCustody};
use crate::observation::{PsiCompilationObserver, PsiCompilationStage};

pub use lowered_psi_to_terminal_psi::ProgramEntryTerminalReceiptError;
use lowered_psi_to_terminal_psi::{prepare_program_entry_receipt, rejoin_program_entry_receipt};

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
pub struct TerminalProductionRequest<'a> {
    /// Portable publication requires closed declaration owners. Invocation code
    /// instead retains local coordinates and cannot enter native production.
    pub publication: terminal_codec::TerminalPublication,
    pub checked: &'a CheckedTrees,
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub machine: TerminalMachineSelection<'a>,
    /// Complete callable/interface roots requested by the owning product.
    /// These retain bodies/declarations without granting execution authority.
    pub roots: crate::TerminalProgramRoots<'a>,
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
            publication: terminal_codec::TerminalPublication::Portable,
            checked,
            exact_toolchain_sources,
            machine,
            roots: crate::TerminalProgramRoots::default(),
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Produce Terminal code beside the custody the caller asked to retain.
    ///
    /// Every product shares one instrumented body: checked-input lowering,
    /// selected optimization, the `ProgramEntry` receipt legs when an entry
    /// identity is supplied, and final retention or portable publication,
    /// including boundary demands and checked body candidates. The source-signature
    /// digest remains opaque; later Omega settlement must compare it with the
    /// retained source signature.
    pub fn produce(
        self,
        custody: TerminalProductionCustody<'_>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ProducedTerminalProgram, TerminalArtifactProductionError> {
        let TerminalProductionCustody { applications } = custody;
        let checked = self.checked;
        let lowered = observer
            .record_result(PsiCompilationStage::Lowering, || {
                lower_machine(
                    self.checked,
                    self.machine,
                    self.exact_toolchain_sources,
                    self.roots,
                    self.publication,
                )
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        // Every consumer observes the same unoptimized composition. A target's
        // refusal cannot erase behavior or reject an independently admitted one.
        let admissions: Vec<_> = applications
            .iter()
            .map(|application| -> Result<_, Vec<Diagnostic>> {
                if application.entry_identity.is_none() && application.admission.is_none() {
                    return Ok(None);
                }
                let selection = observer
                    .record_result(PsiCompilationStage::MachineSelection, || {
                        select_terminal_machine(
                            checked,
                            application.entry_machine.unwrap_or(self.machine),
                        )
                    })
                    .map_err(|error| {
                        TerminalArtifactProductionError::Lowering(error).diagnostics()
                    })?;
                if let Some(admission) = application.admission {
                    for machine in std::iter::once(selection.machine)
                        .chain(application.admission_roots.iter().copied())
                    {
                        let entry = lowered_psi_to_terminal_psi::retained_source_machine(
                            checked,
                            machine,
                            self.exact_toolchain_sources,
                            &lowered.semantic_module,
                        )
                        .map_err(|error| {
                            TerminalArtifactProductionError::EntryReceipt(error).diagnostics()
                        })?;
                        let provenance = lowered_machine_provenance(
                            checked,
                            &lowered,
                            machine,
                            self.exact_toolchain_sources,
                        );
                        admission.admit(&lowered.semantic_module, entry.id, &provenance)?;
                    }
                }
                Ok(Some(selection))
            })
            .collect();
        let optimized = observer
            .record_result(PsiCompilationStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)?;
        let receipts: Vec<_> = applications
            .iter()
            .zip(admissions)
            .map(|(application, admission)| {
                let entry_selection = admission?;
                match (application.entry_identity, entry_selection) {
                    (Some(source_signature_identity), Some(selection)) => observer
                        .record_result(PsiCompilationStage::EntryReceipt, || {
                            prepare_program_entry_receipt(
                                checked,
                                selection,
                                source_signature_identity,
                                self.exact_toolchain_sources,
                                optimized.lowered(),
                            )
                        })
                        .map(Some)
                        .map_err(|error| {
                            TerminalArtifactProductionError::EntryReceipt(error).diagnostics()
                        }),
                    _ => Ok(None),
                }
            })
            .collect();
        let provider_calls =
            crate::build_provider_selections::capture(checked, optimized.lowered())
                .map_err(TerminalArtifactProductionError::BuildContext)?;
        let binding_calls = crate::build_bindings::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let request_calls = crate::build_requests::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let filesystem_calls = crate::build_filesystem::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let program = observer
            .record_result(PsiCompilationStage::Publication, || {
                lowered_psi_to_terminal_psi::finalize_terminal_program(optimized, self.publication)
            })
            .map_err(TerminalArtifactProductionError::Artifact)?;
        let applications = receipts
            .into_iter()
            .map(|receipt| {
                let receipt = receipt?;
                if let Some(receipt) = &receipt {
                    let artifact = program.portable().map_err(|error| {
                        TerminalArtifactProductionError::Artifact(error).diagnostics()
                    })?;
                    rejoin_program_entry_receipt(receipt, artifact).map_err(|error| {
                        TerminalArtifactProductionError::EntryReceipt(error).diagnostics()
                    })?;
                }
                Ok(receipt)
            })
            .collect();
        let build_bindings = crate::TerminalBuildBindings::new(program.view(), binding_calls);
        let build_requests = crate::TerminalBuildRequests::new(program.view(), request_calls);
        let build_filesystem =
            crate::TerminalBuildFilesystem::new(program.view(), filesystem_calls);
        Ok(ProducedTerminalProgram {
            build_provider_selections: crate::TerminalBuildProviderSelections::new(
                program.view(),
                provider_calls,
            ),
            program,
            build_bindings,
            build_requests,
            build_filesystem,
            applications,
        })
    }
}

#[derive(Debug)]
pub enum TerminalArtifactProductionError {
    BuildContext(Diagnostic),
    Lowering(LoweringError),
    Optimization(PsiOptimizationStageError),
    Artifact(terminal_codec::CanonicalTerminalArtifactError),
    EntryReceipt(ProgramEntryTerminalReceiptError),
}

impl TerminalArtifactProductionError {
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        vec![self.diagnostic()]
    }

    /// The refusal as the diagnostic compilation reports. A body this
    /// compiler cannot lower yet reports its plan gap: the construct at its
    /// source span, as an `Incomplete` coverage provision.
    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            Self::BuildContext(error) => Some(error.clone()),
            Self::Lowering(error) => error.plan_gap().cloned(),
            _ => None,
        }
        .unwrap_or_else(|| {
            Diagnostic::error(format!("terminal-artifact production failed: {self}"))
        })
    }
}

diagnostics::debug_display!(TerminalArtifactProductionError);

impl std::error::Error for TerminalArtifactProductionError {}

/// Exact callable and semantic parameter origins after source erasure. Native
/// callers consume this correspondence rather than invoking a Psi stage.
pub struct RetainedCallableSignature {
    pub machine: semantic_vocabulary::MachineId,
    pub scalar_source_positions: Vec<u32>,
    pub structural_source_positions: Vec<u32>,
}

pub fn retained_callable_signature(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    artifact: &terminal_codec::CanonicalTerminalArtifact,
) -> Result<RetainedCallableSignature, Vec<Diagnostic>> {
    let machine = lowered_psi_to_terminal_psi::retained_source_machine(
        checked,
        source,
        exact_toolchain_sources,
        artifact.semantic_module(),
    )
    .map_err(|error| TerminalArtifactProductionError::EntryReceipt(error).diagnostics())?;
    let entry = checked
        .facts
        .flow
        .terminal_unit_effects
        .callable_entry_for_machine(source)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "retained callable has no unique checked entry signature",
            )]
        })?;
    Ok(RetainedCallableSignature {
        machine: machine.id,
        scalar_source_positions: entry
            .scalar_parameters
            .iter()
            .map(|parameter| parameter.source_position)
            .collect(),
        structural_source_positions: entry
            .structural_parameters
            .iter()
            .map(|parameter| parameter.position)
            .collect(),
    })
}
