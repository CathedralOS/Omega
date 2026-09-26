//! Preparation of verified native input from a Terminal artifact and its
//! separately supplied realization authority.

use crate::compiler::native::native_realization::realization_diagnostics::realization_error;
use crate::compiler::native::native_realization::realization_request::{
    NativeRealizationInput, NativeRealizationRequest,
};
use diagnostics::Diagnostic;
use terminal_psi_to_abstract_operations::TerminalPlacedViewEstablishment;

/// Reusable target-neutral lowering of one exact canonical Terminal artifact.
///
/// Construction binds the full artifact identity, exact proof-admission
/// profile, and exact post-Terminal optimization selection. Target selection,
/// provider settlement, authority policy, callbacks, and every
/// physical lowering input remain in each realization request.
#[derive(Debug, Clone)]
pub struct PreparedNativeRealizationInput {
    terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
    profile: proof_admission::AdmissionProfile,
    optimization_selections: optimization_core::PostTerminalOptimizationSelections,
    input: NativeRealizationInput,
}

impl PreparedNativeRealizationInput {
    pub const fn terminal_artifact_identity(&self) -> terminal_codec::TerminalArtifactIdentity {
        self.terminal_artifact_identity
    }

    pub fn admission_profile(&self) -> &proof_admission::AdmissionProfile {
        &self.profile
    }

    pub fn is_optimized(&self) -> bool {
        !self.optimization_selections.is_empty()
    }

    pub fn matches(
        &self,
        terminal_artifact_identity: terminal_codec::TerminalArtifactIdentity,
        profile: &proof_admission::AdmissionProfile,
        optimization_selections: &optimization_core::PostTerminalOptimizationSelections,
    ) -> bool {
        self.terminal_artifact_identity == terminal_artifact_identity
            && self.profile == *profile
            && self.optimization_selections == *optimization_selections
    }

    /// The provider establishments bound to this input's direct-entry
    /// placed-view roster rows, in roster order. Empty for a program that
    /// declared none; the bound set survives `reopen` so every realization
    /// request sees the exact loans preparation admitted.
    pub fn placed_view_establishments(&self) -> &[TerminalPlacedViewEstablishment] {
        self.input.placed_view_establishments()
    }

    pub(crate) fn reopen(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        request: &NativeRealizationRequest<'_>,
    ) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
        if !self.matches(
            artifact.manifest().identity(),
            request.profile,
            request.optimization_selections,
        ) {
            return Err(realization_error(
                "prepared native input",
                "Terminal artifact identity, proof-admission profile, or exact optimization selection changed",
            ));
        }
        Ok(self.input.clone())
    }
}

/// Decode, verify, and lower one canonical Terminal artifact into the reusable
/// target-neutral native input frontier, binding the provider's placed-view
/// supplies: each declared direct-entry roster row joins exactly one
/// establishment, the bound set rides inside the reusable input, and every
/// realization request that reopens this preparation sees the exact admitted
/// loans. A program that declared no rows takes an empty supply. A supply
/// answering no declared row, answering one twice, or carrying a referent
/// that cannot rejoin the module's own catalogs rejects here rather than
/// inside realization — a roster or a pointer is not this authority.
pub fn prepare_native_realization_input(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    profile: &proof_admission::AdmissionProfile,
    optimization_selections: &optimization_core::PostTerminalOptimizationSelections,
    placed_view_establishments: &[TerminalPlacedViewEstablishment],
) -> Result<PreparedNativeRealizationInput, Vec<Diagnostic>> {
    artifact
        .validate()
        .map_err(|error| realization_error("canonical artifact replay", error))?;
    let input = lower_realization_input(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        profile,
        placed_view_establishments,
    )?;
    Ok(PreparedNativeRealizationInput {
        terminal_artifact_identity: artifact.manifest().identity(),
        profile: profile.clone(),
        optimization_selections: optimization_selections.clone(),
        input,
    })
}

pub(crate) fn lower_realization_input(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    placed_view_establishments: &[TerminalPlacedViewEstablishment],
) -> Result<NativeRealizationInput, Vec<Diagnostic>> {
    // This leg requires the sealed PSIPSC proof section: the section must name
    // the identity reconstructed from this exact module, so a bare proof
    // bundle or a section sealed to another subject is rejected here rather
    // than inside the still-transitional admission decode beneath.
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(|error| realization_error("semantic section", error))?;
    terminal_codec::decode_proof_section_for(&module, proof_bytes)
        .map_err(|error| realization_error("proof section", error))?;
    terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes,
            proof_bytes,
            obligation_ledger_bytes: None,
        },
        profile,
    )
    .and_then(|admitted| admitted.try_into_native_input(placed_view_establishments))
    .map_err(|error| realization_error("native artifact lowering", error))
}
