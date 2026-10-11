//! Emit the requested image and replay the corresponding native artifact.
//!
//! The image writer selects the authority-distinct result. Both branches
//! consume the same source-free evidence, then use their own construction and
//! independent replay; dynamic ELF never acquires direct-image authority.

use super::{DynamicElfNativeArtifact, NativeArtifact, NativeArtifactEmissionParts};
use diagnostics::Diagnostic;
use image_emission::{ExecutableImageEmissionRequest, RequestedExecutableImage};

/// Source-free native result selected by the exact object-bound image request.
/// Dynamic ELF remains a separate, non-installable authority class.
#[derive(Debug)]
#[must_use = "requested native realization retains its authority-distinct image custody"]
pub enum RequestedNativeArtifact {
    Direct(NativeArtifact),
    DynamicElf(DynamicElfNativeArtifact),
}

impl RequestedNativeArtifact {
    /// Bind the published object to the exact selected plans and admitted
    /// provider evidence before emitting an image. Installation is consumed
    /// by target lowering; this captures the resulting execution and authority
    /// evidence, then independently replays it against the emitted product.
    /// Compact report identities never replace the full selection digest.
    pub fn emit_admitted(
        target: target::NativeTarget,
        terminal: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
        published: crate::PublishedFragmentObject,
        image: ExecutableImageEmissionRequest,
        selected: &effects::SelectedProviderPlanFacts,
        providers: terminal_psi_to_abstract_operations::provider_admission::AdmittedNativeProviders<
            '_,
        >,
        boundary_application_coverage: Option<
            boundary_applications::TerminalBoundaryApplicationCoverage,
        >,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::emit(NativeArtifactEmissionParts {
            target,
            psi_artifact: terminal,
            object: published.object,
            image,
            selected_provider_closure_report_identity: selected.compatibility_report_identity(),
            selected_provider_closure_digest:
                super::NativeSelectedProviderClosureDigest::from_digest(
                    *selected.identity_digest().as_bytes(),
                ),
            selected_provider_plans: super::NativeSelectedProviderPlan::project_selection(selected),
            provider_executions: providers
                .executions
                .into_iter()
                .map(super::NativeProviderExecution::from_evidence)
                .collect(),
            terminal_authority_policy_identity: providers.terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity: providers
                .terminal_authority_permission_policy_identity,
            terminal_authority_closure_review: providers.terminal_authority_closure_review,
            boundary_application_coverage,
            physical_evidence_scope: published.physical_evidence_scope,
        })
    }

    /// Emit an image from the retained object and construct its complete
    /// Terminal/object/image custody. Every physical-evidence derivation and
    /// replay runs before a product is returned. This writes no files and
    /// grants no receiving, installation or execution authority.
    pub fn emit(
        parts: NativeArtifactEmissionParts<ExecutableImageEmissionRequest>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let NativeArtifactEmissionParts {
            target,
            psi_artifact,
            object,
            image: image_request,
            selected_provider_closure_report_identity,
            selected_provider_closure_digest,
            selected_provider_plans,
            provider_executions,
            terminal_authority_policy_identity,
            terminal_authority_permission_policy_identity,
            terminal_authority_closure_review,
            boundary_application_coverage,
            physical_evidence_scope,
        } = parts;
        let image = image_emission::emit_executable_image(&object, image_request)
            .map_err(|error| vec![error.diagnostic().clone()])?;
        match image {
            RequestedExecutableImage::Direct(image) => {
                NativeArtifact::from_emitted_parts(NativeArtifactEmissionParts {
                    target,
                    psi_artifact,
                    object,
                    image,
                    selected_provider_closure_report_identity,
                    selected_provider_closure_digest,
                    selected_provider_plans,
                    provider_executions,
                    terminal_authority_policy_identity,
                    terminal_authority_permission_policy_identity,
                    terminal_authority_closure_review,
                    boundary_application_coverage,
                    physical_evidence_scope,
                })
                .map(Self::Direct)
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "native artifact native artifact replay failed: {error}"
                    ))]
                })
            }
            RequestedExecutableImage::DynamicElf(image) => {
                DynamicElfNativeArtifact::from_emitted_parts(NativeArtifactEmissionParts {
                    target,
                    psi_artifact,
                    object,
                    image,
                    selected_provider_closure_report_identity,
                    selected_provider_closure_digest,
                    selected_provider_plans,
                    provider_executions,
                    terminal_authority_policy_identity,
                    terminal_authority_permission_policy_identity,
                    terminal_authority_closure_review,
                    boundary_application_coverage,
                    physical_evidence_scope,
                })
                .map(Self::DynamicElf)
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "native artifact dynamic ELF native artifact replay failed: {error}"
                    ))]
                })
            }
        }
    }
}
