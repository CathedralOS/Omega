//! Complete Terminal-to-native admission before abstract optimization.
//!
//! This stage owns the ordering of canonical replay, source-free custody,
//! provider installation and mechanism review. The compiler supplies policy
//! and consumes the result; it does not rebuild this admission between stages.

use crate::provider_admission::{
    AdmittedNativeProviders, ProviderAdmissionRequest, admit_native_providers,
};
use crate::{ArtifactLoweringError, PreparedNativeRealizationInput, VerifiedNativeArtifactInput};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope;
use terminal_codec::CanonicalTerminalArtifact;

/// Independent native custody beside one canonical Terminal product.
pub struct NativeAdmissionRequest<'request> {
    /// Exact provider and entry custody plus the receiving authority policies.
    /// An absent permission policy makes no receiver-admission claim; it is
    /// neither an empty policy nor permission inferred from accepted packages.
    pub providers: ProviderAdmissionRequest<'request>,
    /// Same-artifact source custody. Nonempty scope requires exact coverage.
    pub checked_scope: Option<&'request CheckedBoundaryOperatorApplicationScope>,
    /// Target-neutral reuse; reopening checks exact artifact/profile equality.
    pub prepared_input: Option<&'request PreparedNativeRealizationInput>,
    pub boundary_application_coverage: Option<&'request TerminalBoundaryApplicationCoverage>,
    /// Retained exclusions also constrain physical authority when no receiving
    /// permission policy is supplied.
    pub behavior_exclusions: &'request effects::BehaviorExclusions,
}

/// Complete stage-00 native input. No physical work or publication has occurred.
pub struct AdmittedNativeRealization<'execution> {
    pub input: VerifiedNativeArtifactInput,
    pub providers: AdmittedNativeProviders<'execution>,
    pub boundary_application_coverage: Option<TerminalBoundaryApplicationCoverage>,
}

pub fn admit_native_realization<'request>(
    artifact: &CanonicalTerminalArtifact,
    request: &NativeAdmissionRequest<'request>,
) -> Result<AdmittedNativeRealization<'request>, Vec<Diagnostic>> {
    if let Some(scope) = request.checked_scope {
        scope
            .validate_for_artifact(artifact)
            .map_err(|error| realization_error("checked boundary-operator scope", error))?;
    }
    request
        .providers
        .program_entry
        .validate_for_target(request.providers.target)
        .map_err(|error| realization_error("ProgramEntry custody", error))?;
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    crate::validate_fused_program_entry_establishments(
        artifact,
        request.providers.program_entry,
        request.providers.selected_provider_plans,
    )
    .map_err(|error| realization_error("Fused ProgramEntry establishment", error))?;
    let boundary_application_coverage = crate::retain_terminal_boundary_application_coverage(
        artifact,
        request.checked_scope,
        request.boundary_application_coverage,
    )
    .map_err(|error| realization_error("boundary-application custody", error))?;
    let input = match request.prepared_input {
        Some(prepared) => prepared
            .reopen(artifact, request.providers.profile)
            .map_err(|error| realization_error("prepared native input", error))?,
        None => VerifiedNativeArtifactInput::prepare(artifact, request.providers.profile, &[])
            .map_err(artifact_lowering_error)?,
    };
    let providers = admit_native_providers(&input, artifact, &request.providers)?;
    // Exclusions constrain the exercised mechanism closure even when the
    // caller makes no receiver-permission claim.
    request
        .behavior_exclusions
        .validate_physical_authority_closure(&providers.terminal_authority_closure_review)?;
    Ok(AdmittedNativeRealization {
        input,
        providers,
        boundary_application_coverage,
    })
}

pub(crate) fn artifact_lowering_error(error: ArtifactLoweringError) -> Vec<Diagnostic> {
    match error {
        ArtifactLoweringError::CanonicalArtifact(error) => {
            realization_error("canonical artifact replay", error)
        }
        ArtifactLoweringError::SemanticDecode(error) => {
            realization_error("semantic section", error)
        }
        ArtifactLoweringError::ProofDecode(error) => realization_error("proof section", error),
        error => realization_error("native artifact lowering", error),
    }
}

fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}
