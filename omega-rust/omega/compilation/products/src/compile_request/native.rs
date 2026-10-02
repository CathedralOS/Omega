use diagnostics::Diagnostic;
use image_emission::ExecutableImageEmissionRequest;
use optimization_core::OptimizationRollback;
use terminal_psi_to_abstract_operations::provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};

/// One target's native product request: where the source root is, the
/// admission profile the Terminal artifact is verified under, the receiving
/// mechanism-classification policy, the optional service-permission admission
/// policy, and the optimization rollback to settle.
///
/// The terminal-authority policy is the receiving authority's explicit
/// mechanism table: the closed compiler-intrinsic inventory classifies itself,
/// while normalized-foreign, syscall, and checked-physical leaves may need
/// explicit rows. Ordinary production retains unclassified mechanisms; receiver
/// admission and physical exclusions require complete classification. This is an independent axis --
/// trust admissions, service permissions, and provider execution custody
/// never substitute for or widen its rows.
///
/// `terminal_authority_permission_policy` is the second, receiver-admission
/// axis. `None` means ordinary production makes no receiver-admission claim:
/// accepted package permissions are never projected into receiver rows, and
/// the emitted artifact cannot satisfy an explicit admission replay. `Some`
/// (including an explicit empty policy) adjudicates every closure leaf
/// against exact supplied rows.
pub struct NativeProductRequest {
    pub root_path: std::path::PathBuf,
    pub terminal_admission_profile: proof_admission::AdmissionProfile,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub optimization_rollback: OptimizationRollback,
}

/// Native admission plus the product's physical selections and image destination.
/// Stage 00 owns admission vocabulary and checks; this request supplies policy
/// and retains the exact image custody if realization fails.
pub struct NativeRealizationRequest<'request> {
    pub admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'request>,
    pub image_request: ExecutableImageEmissionRequest,
    /// The retained proposal's effective ELF loader, settled against the
    /// emitted object: it becomes `PT_INTERP` only when that object links
    /// dynamically, and a static object ignores it.
    pub elf_interpreter: Option<&'request target::NormalizedElfInterpreterPlan>,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    /// Isolated bodies paired one-to-one with admitted callback arguments.
    /// Each body has its own Terminal artifact namespace.
    pub callback_thunks: &'request [backend_plan::NativeCallbackThunkSettlement<'request>],
}

impl NativeRealizationRequest<'_> {
    /// Product-owned per-pass ceilings, identical for the program and callbacks.
    /// Identity staging meters traversal as well as optimizer iterations.
    pub fn optimization_budget(&self) -> optimization_core::OptimizationWorkBudget {
        // Spill validation charges whole-plan traversal and slot-liveness scans,
        // not just changed instructions. A complete application exceeds 100,000
        // steps while recovering register pressure. Callee-save derivation also
        // charges functions, blocks, instructions, operands and writes as both
        // validation steps and iterations. Give both traversal counters room
        // without changing candidate/commit limits or bypassing independent replay.
        optimization_core::OptimizationWorkBudget::new(
            1_000_000, 100_000, 1_000_000, 100_000, 1_000_000,
        )
        .expect("compiler baseline optimizer ceilings are nonzero")
    }

    /// Return the caller's complete image input without substituting a carrier.
    pub fn into_failure(self, diagnostics: Vec<Diagnostic>) -> RequestedNativeArtifactError {
        RequestedNativeArtifactError {
            image_request: self.image_request,
            diagnostics,
        }
    }
}

/// Explicit policy, image custody and admitted imports for retained re-entry.
/// Policies do not establish package admission: the package consumer binds
/// accepted evidence to the production subject before constructing this input.
pub struct RetainedNativeRealizationRequest<'request> {
    pub profile: &'request proof_admission::AdmissionProfile,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    /// This is the independent receiver-admission axis. `None` makes no claim;
    /// it never means an empty policy or permissions inferred from packages.
    /// `Some`, including an empty policy, adjudicates every closure leaf.
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub image_request: ExecutableImageEmissionRequest,
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
}

impl RetainedNativeRealizationRequest<'_> {
    /// Require native proposal custody and rejoin the image's loader/signing
    /// metadata before provider preparation. This admits a product input, not
    /// provider, receiver or execution authority; stage 00 still checks those.
    /// The caller retains its original image request for failure recovery.
    pub fn prepare_terminal(
        &mut self,
        retained: crate::RetainedTerminalArtifact,
    ) -> Result<
        (
            terminal_codec::CanonicalTerminalArtifact,
            Vec<backend_plan::BoundNominalCallbackPlacement>,
            crate::TerminalNativeRealizationProposal,
        ),
        Vec<Diagnostic>,
    > {
        retained
            .validate()
            .map_err(|message| diagnostic("retained Terminal product", message))?;
        let (artifact, callback_placements, proposal) = retained.into_parts();
        let proposal = proposal.ok_or_else(|| {
            diagnostic(
                "retained Terminal product",
                "source-evaluated import realization requires one native proposal",
            )
        })?;
        self.image_request
            .settle_application_metadata(
                proposal.native_target(),
                proposal.application().intent,
                proposal.application().subsystem,
                proposal.application().identifier.as_ref(),
            )
            .map_err(|error| {
                use image_emission::ImageApplicationMetadataError;
                diagnostic(
                    "Terminal native proposal",
                    match error {
                        ImageApplicationMetadataError::SubsystemMismatch =>
                            "image request subsystem differs from the retained build subsystem",
                        ImageApplicationMetadataError::IdentifierMismatch =>
                            "image request code signature identifier differs from the retained build identifier",
                        ImageApplicationMetadataError::MissingGuiIdentifier =>
                            "signed macOS GUI image emission requires the retained build identifier",
                    },
                )
            })?;
        Ok((artifact, callback_placements, proposal))
    }
}

/// Failed realization with the complete image input recoverable.
#[derive(Debug)]
#[must_use = "requested native realization failure retains the exact image request"]
pub struct RequestedNativeArtifactError {
    image_request: ExecutableImageEmissionRequest,
    diagnostics: Vec<Diagnostic>,
}

impl RequestedNativeArtifactError {
    pub fn into_parts(self) -> (ExecutableImageEmissionRequest, Vec<Diagnostic>) {
        (self.image_request, self.diagnostics)
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
