//! Complete Terminal-to-native admission before abstract optimization.
//!
//! This stage owns the ordering of canonical input preparation, provider
//! settlement, installation and mechanism review. The entry, fused-service and
//! boundary-coverage custody arrive from the native proposal this compile
//! derived for the same artifact and are not replayed here. The compiler
//! supplies policy and consumes the result; it does not rebuild this
//! admission between stages.

use crate::provider_admission::{
    AdmittedNativeProviders, ProviderAdmissionRequest, admit_native_providers,
};
use crate::{ArtifactLoweringError, VerifiedNativeArtifactInput};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use diagnostics::Diagnostic;
use terminal_codec::CanonicalTerminalArtifact;

/// Independent native custody beside one canonical Terminal product.
pub struct NativeAdmissionRequest<'request> {
    /// Exact provider and entry custody plus the receiving authority policies.
    /// An absent permission policy makes no receiver-admission claim; it is
    /// neither an empty policy nor permission inferred from accepted packages.
    pub providers: ProviderAdmissionRequest<'request>,
    /// The boundary-application coverage the proposal's producer settled
    /// against this artifact's sealed checked scope.
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

/// Admit the artifact's canonical input and its providers. The entry
/// settlement, fused establishments and boundary coverage arrive from the
/// proposal this compile derived for this artifact, so admission does not
/// replay their artifact joins again (pipeline.md, "Verification placement").
pub fn admit_native_realization<'request>(
    artifact: &CanonicalTerminalArtifact,
    request: &NativeAdmissionRequest<'request>,
) -> Result<AdmittedNativeRealization<'request>, Vec<Diagnostic>> {
    let input =
        VerifiedNativeArtifactInput::prepare(artifact, &[]).map_err(artifact_lowering_error)?;
    let providers = admit_native_providers(&input, artifact, &request.providers)?;
    // Exclusions constrain the exercised mechanism closure even when the
    // caller makes no receiver-permission claim.
    request
        .behavior_exclusions
        .validate_physical_authority_closure(&providers.terminal_authority_closure_review)?;
    Ok(AdmittedNativeRealization {
        input,
        providers,
        boundary_application_coverage: request.boundary_application_coverage.cloned(),
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
        error => realization_error("native artifact lowering", error),
    }
}

fn realization_error(context: &str, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "native artifact {context} failed: {error}"
    ))]
}
