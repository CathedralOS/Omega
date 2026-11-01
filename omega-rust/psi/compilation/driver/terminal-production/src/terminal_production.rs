//! Sequence the selected Psi lowering, optimization and publication owners.

use checked_trees::CheckedTrees;
use checked_trees_to_lowered_psi::{LoweringError, TerminalMachineSelection, lower_machine};
use diagnostics::Diagnostic;
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};

use crate::custody::{ProducedTerminalProgram, TerminalProductionCustody};
use crate::observation::{PsiCompilationObserver, PsiCompilationStage};

pub use lowered_psi_to_terminal_psi::ProgramEntryTerminalReceiptError;

use crate::build_bindings::capture_native_formations;

/// Exact borrowed source inputs and target-neutral selection for Terminal production.
pub struct TerminalProductionRequest<'a> {
    /// Portable publication requires closed declaration owners. Invocation code
    /// instead retains local coordinates and cannot enter native production.
    pub publication: terminal_codec::TerminalPublication,
    pub checked: &'a CheckedTrees,
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    /// Reuse the declaration product already owned by this exact checked input.
    pub external_declarations: Option<&'a crate::PreparedExternalProviderDeclarations>,
    /// The completed Product owner's prepared declaration census. Early Build
    /// jobs have no Product inventory and publish no Product calling telescope.
    pub provider_inventory: Option<&'a checked_trees::PreparedProviderInventory>,
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
            external_declarations: None,
            provider_inventory: None,
            machine,
            roots: crate::TerminalProgramRoots::default(),
            optimization_selections: optimization::PsiOptimizationSelections::default(),
        }
    }

    /// Produce Terminal code beside the custody the caller asked to retain.
    ///
    /// Every product shares one instrumented body: checked-input lowering,
    /// selected optimization and final retention or portable publication,
    /// including boundary demands and checked body candidates. The source-signature
    /// digest remains opaque; later Omega settlement must compare it with the
    /// retained source signature. Source-free application descriptions and
    /// requested `ProgramEntry` receipts attach only after publication, never
    /// while lowering or optimizing shared code.
    pub fn produce(
        self,
        custody: TerminalProductionCustody<'_>,
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ProducedTerminalProgram, TerminalArtifactProductionError> {
        let TerminalProductionCustody { applications } = custody;
        let checked = self.checked;
        let declaration_requests = crate::build_bindings::capture_declarations(checked);
        let native_formations =
            capture_native_formations(checked, self.machine, &declaration_requests)
                .map_err(TerminalArtifactProductionError::BuildContext)?;
        let mut lowered = observer
            .record_result(PsiCompilationStage::Lowering, || {
                lower_machine(
                    self.checked,
                    self.machine,
                    self.exact_toolchain_sources,
                    self.roots,
                    self.publication,
                    self.external_declarations,
                )
            })
            .map_err(TerminalArtifactProductionError::Lowering)?;
        if let Some(inventory) = self.provider_inventory {
            crate::calling_signatures::capture(
                checked,
                inventory,
                &mut lowered.semantic_module,
                &lowered.source_boundary_declarations,
                &lowered.source_machine_declarations,
            )
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        }
        lowered.semantic_module.unoptimized_behavior = lowered
            .semantic_module
            .machines
            .iter()
            .map(terminal_psi::UnoptimizedMachineBehavior::capture)
            .collect();
        // Attribution is captured once while lowering owns source joins.
        // Requested roots attach to the sealed artifact after publication.
        let provenance = checked_trees_to_lowered_psi::lowered_module_provenance(checked, &lowered);
        let optimized = observer
            .record_result(PsiCompilationStage::Optimization, || {
                run_psi_optimization(lowered, self.optimization_selections)
            })
            .map_err(TerminalArtifactProductionError::Optimization)?;
        let binding_calls = crate::build_bindings::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let request_calls = crate::build_requests::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let filesystem_calls = crate::build_filesystem::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let query_calls = crate::build_queries::capture(checked, optimized.lowered())
            .map_err(TerminalArtifactProductionError::BuildContext)?;
        let program = observer
            .record_result(PsiCompilationStage::Publication, || {
                lowered_psi_to_terminal_psi::finalize_terminal_program(optimized, self.publication)
            })
            .map_err(TerminalArtifactProductionError::Artifact)?;
        let applications = applications
            .iter()
            .map(|application| {
                application.attach(program.view(), program.portable(), &provenance, observer)
            })
            .collect();
        let build_bindings = crate::TerminalBuildBindings::new(
            program.view(),
            binding_calls,
            declaration_requests,
            query_calls,
            native_formations,
        );
        let build_requests = crate::TerminalBuildRequests::new(program.view(), request_calls);
        let build_filesystem =
            crate::TerminalBuildFilesystem::new(program.view(), filesystem_calls);
        Ok(ProducedTerminalProgram {
            program,
            build_bindings,
            build_requests,
            build_filesystem,
            applications,
            provenance,
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
