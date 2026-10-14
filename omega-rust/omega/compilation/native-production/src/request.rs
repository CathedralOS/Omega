use diagnostics::Diagnostic;
use image_emission::ExecutableImageEmissionRequest;
use terminal_psi_to_abstract_operations::provider_admission::{
    SourceEvaluatedImportSettlement, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};

/// Native admission plus the product's physical selections and image destination.
/// Stage 00 owns admission vocabulary and checks; this request supplies policy
/// and retains the exact image custody if realization fails.
pub(crate) struct NativeRealizationRequest<'request> {
    /// Application-entry custody is consumed only by receiver/image production,
    /// not by ordinary provider admission of private canonical bodies.
    pub program_entry: terminal_psi_to_abstract_operations::NativeProgramEntrySettlement<'request>,
    pub admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'request, 'request>,
    pub image_request: ExecutableImageEmissionRequest,
    /// The retained proposal's effective ELF loader, settled against the
    /// emitted object: it becomes `PT_INTERP` only when that object links
    /// dynamically, and a static object ignores it.
    pub elf_interpreter: Option<&'request target::NormalizedElfInterpreterPlan>,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    /// Exact callable selections paired with admitted callback arguments,
    /// borrowing the shared program and its ordinary provider evidence.
    pub callback_thunks: &'request [backend_plan::NativeCallbackThunkSettlement<'request>],
    pub target_provider_proposals: &'request [backend_plan::TerminalTargetProviderProposal],
}

impl NativeRealizationRequest<'_> {
    /// Product-owned per-pass ceilings, identical for the program and callbacks.
    /// Identity staging meters traversal as well as optimizer iterations.
    pub fn optimization_budget(&self) -> optimization_core::OptimizationWorkBudget {
        // A spill charges the traversal of the function it rewrites, one step
        // per plan function, and slot-liveness scans; a complete application
        // exceeds 100,000 steps while recovering register pressure in one
        // function. Callee-save derivation charges functions, blocks,
        // instructions, operands and writes as both validation steps and
        // iterations, and only its derived requirements as candidates. These
        // ceilings are calibrated for one `OPTIMIZATION_REFERENCE_EXTENT` of
        // input: the required recovery rewrites and the callee-save
        // derivations scale them with the function or program they read, so
        // linear work never caps the size of a program. The validation
        // ceiling must hold the costliest linear audit at the reference
        // extent: the fixed-occurrence audit charges each operand about its
        // register depth plus 16 steps and each successor-edge transport
        // about twice that depth plus 4, around 36 steps per unit at a 2^16
        // extent, so 2^16 units need about 2.4 million steps. The product
        // typing harness's largest function (14,703 operands, 33,342
        // transports) needed 1,062,206 against the earlier 1,000,000. Give
        // both traversal counters room without changing candidate/commit
        // limits or bypassing independent replay.
        optimization_core::OptimizationWorkBudget::new(
            1_000_000, 100_000, 4_000_000, 100_000, 1_000_000,
        )
        .expect("compiler baseline optimizer ceilings are nonzero")
    }
}

/// Explicit policy, image custody and admitted imports for retained re-entry.
/// Policies do not establish package admission: the package consumer binds
/// accepted evidence to the production subject before constructing this input.
pub struct NativeCompileRequest<'request> {
    pub private_entry_ram: &'request [terminal_installation::PrivateEntryRamInitialization],
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy: TerminalAuthorityPermissionPolicy,
    /// This is the independent receiver-admission axis. `None` makes no claim;
    /// it never means an empty policy or permissions inferred from packages.
    /// `Some`, including an empty policy, adjudicates every closure leaf.
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub image_request: ExecutableImageEmissionRequest,
    /// A nonempty roster covers the selected application/callback import union
    /// exactly once. Empty means derive from the selected plans, not fill holes
    /// in supplied custody. Private bodies borrow their own demand projection.
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
}

impl NativeCompileRequest<'_> {
    /// Require native proposal custody and rejoin the image's loader/signing
    /// metadata before provider preparation. This admits a product input, not
    /// provider, receiver or execution authority; stage 00 still checks those.
    /// The retained product never left this process, so its proposal is not
    /// replayed against the artifact again.
    /// The caller retains its original image request for failure recovery.
    pub(crate) fn prepare_terminal(
        &mut self,
        retained: terminal_psi_to_abstract_operations::RetainedTerminalArtifact,
    ) -> Result<
        (
            std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
            terminal_psi_to_abstract_operations::TerminalNativeRealizationProposal,
        ),
        Vec<Diagnostic>,
    > {
        let (artifact, proposal) = retained.into_parts();
        let proposal = proposal.ok_or_else(|| {
            diagnostics::error_in_context(
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
                diagnostics::error_in_context(
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
        Ok((artifact, proposal))
    }
}
