use super::error::ProviderInstallationError;
use super::model::{AdmittedProviderInstallation, SelectedProviderAdapter};
use super::replay::replay_installed_provider_calls;
use crate::artifact::ArtifactLoweringError;
use crate::lowering::lower_decoded_verified_module;
use crate::shared::*;

pub(super) fn admit_provider_installation_with_projection(
    plan: &AbstractOperationPlan,
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    selected: &[SelectedProviderAdapter],
    retain_payloadless_for_optimization: bool,
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)
        .map_err(ProviderInstallationError::ArtifactReplay)?;
    let proof = terminal_codec::decode_proof_bundle(proof_bytes)
        .map_err(ArtifactLoweringError::ProofDecode)
        .map_err(ProviderInstallationError::ArtifactReplay)?;
    let verified = terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ArtifactLoweringError::Verification)
        .map_err(ProviderInstallationError::ArtifactReplay)?;
    let replayed = lower_decoded_verified_module(&verified, retain_payloadless_for_optimization)
        .map_err(ArtifactLoweringError::Lowering)
        .map_err(ProviderInstallationError::ArtifactReplay)?;
    if &replayed != plan {
        return Err(ProviderInstallationError::PlanReplayMismatch);
    }
    let mut selections = Vec::new();
    let mut installed_candidates = Vec::new();
    let mut boundaries = plan
        .provider_candidates
        .iter()
        .map(|candidate| candidate.boundary)
        .collect::<Vec<_>>();
    boundaries.sort();
    boundaries.dedup();
    for boundary in boundaries {
        let candidates = plan
            .provider_candidates
            .iter()
            .filter(|candidate| candidate.boundary == boundary)
            .collect::<Vec<_>>();
        let requirement_identity = candidates[0].requirement_identity.as_str();
        if requirement_identity.is_empty()
            || candidates
                .iter()
                .any(|candidate| candidate.requirement_identity != requirement_identity)
        {
            return Err(ProviderInstallationError::InvalidLoweredCatalog);
        }
        let selected_rows = selected
            .iter()
            .filter(|row| row.requirement_identity == requirement_identity)
            .map(|row| {
                (
                    row.provider_identity.as_str(),
                    row.machine_identity.as_str(),
                )
            })
            .collect::<Vec<_>>();
        if selected_rows.is_empty() {
            return Err(ProviderInstallationError::MissingSelectedProvider { boundary });
        }
        let exact = candidates
            .iter()
            .filter(|candidate| {
                selected_rows.iter().any(|(provider, machine)| {
                    candidate.provider_identity == *provider
                        && candidate.candidate_identity == *machine
                })
            })
            .collect::<Vec<_>>();
        let [candidate] = exact.as_slice() else {
            return Err(if exact.is_empty() {
                ProviderInstallationError::SelectedProviderMismatch { boundary }
            } else {
                ProviderInstallationError::AmbiguousSelectedProvider { boundary }
            });
        };
        selections.push(terminal_interpreter::ProviderInstallationSelection {
            boundary,
            provider_identity: candidate.provider_identity.clone(),
            candidate: candidate.candidate,
        });
        installed_candidates.push((**candidate).clone());
    }
    let installed_calls =
        replay_installed_provider_calls(plan, verified.module(), &installed_candidates)?;
    let installation = terminal_interpreter::admit_provider_installation_from_artifact(
        semantic_bytes,
        proof_bytes,
        profile,
        &selections,
    )
    .map_err(ProviderInstallationError::PsiAdmission)?;
    if installation.terminal_psi() != plan.psi {
        return Err(ProviderInstallationError::TerminalIdentityMismatch);
    }
    Ok(AdmittedProviderInstallation {
        psi: installation.terminal_psi(),
        psi_installation: installation,
        installed_candidates,
        installed_calls,
    })
}
