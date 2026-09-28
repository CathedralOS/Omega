//! Native re-entry for a retained Terminal product with source-evaluated imports.
//!
//! Provider admission owns the exact-plan join. Callers supply only independently
//! admitted execution and same-stack evidence; they cannot select a provider
//! plan, locator, target, compiler builtin, or checked-scope receipt.

use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::provider_admission::ProviderAdmissionRequest;
pub use terminal_psi_to_abstract_operations::provider_admission::SourceEvaluatedImportSettlement;
use terminal_psi_to_abstract_operations::{NativeAdmissionRequest, RetainedNativeProviderInputs};

/// Explicit policy, image custody, and admitted imports for native re-entry.
///
/// Policies do not establish package admission. Package orchestration must bind
/// opaque accepted evidence to the retained production subject and derive the
/// exact accepted-package policy before constructing this request.
///
/// `terminal_authority_permission_policy` is the receiver-admission axis and
/// stays optional: `None` means this production makes no receiver-admission
/// claim — accepted package rows are never silently projected into receiver
/// permissions, and the emitted artifact cannot satisfy an explicit admission
/// replay. `Some` (including an explicit empty policy) rejoins every accepted
/// row and adjudicates every closure leaf against it.
pub struct RetainedNativeRealizationRequest<'request> {
    pub profile: &'request proof_admission::AdmissionProfile,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    pub terminal_authority_policy: crate::native::TerminalAuthorityPolicy,
    pub accepted_package_terminal_authority_permission_policy:
        crate::native::TerminalAuthorityPermissionPolicy,
    pub terminal_authority_permission_policy:
        Option<crate::native::TerminalAuthorityPermissionPolicy>,
    pub image_request: crate::native::ExecutableImageEmissionRequest,
    pub imports: &'request [SourceEvaluatedImportSettlement<'request>],
}

/// Consume a retained Terminal product into the requested non-installing native
/// carrier. Rejection returns the complete image input, including its interpreter.
///
/// Package permissions must exactly match the retained proposal. A supplied
/// receiving policy may add unrelated rows, but cannot omit or alter accepted
/// rows; when no receiving policy is supplied this production makes no
/// receiver-admission claim at all. Imports are independently rejoined to
/// their exact retained provider plans.
pub fn realize_retained_native_artifact(
    retained: crate::report::RetainedTerminalArtifact,
    request: RetainedNativeRealizationRequest<'_>,
) -> Result<
    crate::native::RequestedNativeArtifact,
    (
        crate::native::ExecutableImageEmissionRequest,
        Vec<Diagnostic>,
    ),
> {
    let RetainedNativeRealizationRequest {
        profile,
        optimization_selections,
        terminal_authority_policy,
        accepted_package_terminal_authority_permission_policy,
        terminal_authority_permission_policy,
        image_request,
        imports,
    } = request;
    let recoverable_image_request = image_request.clone();
    let result = (|| {
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
        let mut image_request = image_request;
        image_request
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
        let prepared = proposal.prepare_native_providers(
            &artifact,
            &callback_placements,
            RetainedNativeProviderInputs {
                optimization_selections,
                accepted_package_permissions:
                    &accepted_package_terminal_authority_permission_policy,
                receiving_permissions: terminal_authority_permission_policy.as_ref(),
                imports,
            },
        )?;
        let native_settlements = prepared.imports.settlements();
        crate::native::realize_native_artifact(
            artifact,
            crate::native::NativeRealizationRequest {
                admission: NativeAdmissionRequest {
                    checked_scope: Some(&proposal.boundary_applications().scope),
                    prepared_input: None,
                    boundary_application_coverage: Some(&proposal.boundary_applications().coverage),
                    behavior_exclusions: proposal.behavior_exclusions(),
                    providers: ProviderAdmissionRequest {
                        target: proposal.native_target(),
                        profile,
                        program_entry: proposal.program_entry().as_settlement(),
                        selected_provider_plans: &proposal.providers().selection,
                        external_binding_rows: proposal.providers().external_bindings.as_slice(),
                        settlements: &native_settlements,
                        compiler_builtins: &prepared.compiler_builtins,
                        native_callbacks: &prepared.callbacks,
                        terminal_authority_policy: &terminal_authority_policy,
                        terminal_authority_permission_policy: &terminal_authority_permission_policy,
                    },
                },
                image_request,
                optimization_selections,
                callback_thunks: &prepared.callback_thunks,
            },
        )
        .map_err(|error| error.into_parts().1)
    })();
    result.map_err(|diagnostics| (recoverable_image_request, diagnostics))
}

fn diagnostic(context: &str, message: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!("{context}: {message}"))]
}
