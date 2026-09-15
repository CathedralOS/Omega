//! The public entries: interpreting a verified artifact under an admission
//! profile, with or without an effect handler, structural inputs and a fuel
//! measure, and admitting a provider installation from an artifact.

use crate::TerminalStructuralCaseValue;
use crate::TerminalStructuralScalarFieldValue;
use crate::terminal_interpreter::{
    AcceptTerminalEffects, AdmittedProviderInstallation, MeasuredTerminalExecution,
    ProviderInstallationError, ProviderInstallationSelection, TerminalArtifactInterpretError,
    TerminalEffectHandler, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalScalarValue, TerminalStructuralBooleanFieldValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use std::collections::BTreeMap;
use terminal_fuel::{FuelMeterError, TerminalFuelMeter};

/// Exact initialized contents supplied by the embedding host for structural
/// entry arguments. All paths remain rooted in the original referents.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalStructuralInputs<'input> {
    pub arguments: &'input [TerminalStructuralValue],
    pub scalar_fields: &'input [TerminalStructuralScalarFieldValue],
    pub primitive_values: &'input [TerminalStructuralPrimitiveValue],
    pub cases: &'input [TerminalStructuralCaseValue],
}

/// Decode, verify, and execute the canonical semantic and proof sections of one
/// terminal-Psi artifact. This is the reference-interpreter trust boundary for
/// executable artifact content: no source, checked tree, producer-owned module,
/// or prevalidated Rust object crosses it. Installation and debug sections are
/// separately bound by the artifact manifest and do not affect interpretation.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut handler = AcceptTerminalEffects;
    interpret_terminal_artifact_with_effect_handler_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        arguments,
        &[],
        &mut handler,
    )
}

/// Decode one complete portable Terminal-Psi envelope, independently verify
/// its semantic and proof sections, and execute it with a fresh effect-policy
/// input supplied by the receiver. The envelope contains no checked-tree or
/// build-process object.
pub fn interpret_serialized_terminal_artifact_with_effect_handler_measured(
    artifact_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(artifact_bytes)
        .map_err(TerminalArtifactInterpretError::ArtifactDecode)?;
    interpret_terminal_artifact_with_effect_handler_measured(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        profile,
        scalar_arguments,
        structural_arguments,
        handler,
    )
}

/// Execute one verified artifact with opaque structural runtime arguments and
/// an injected deterministic effect handler. The interpreter records every
/// accepted effect in semantic execution order; the handler cannot inspect or
/// mutate fuel, values, claims, or control state.
pub fn interpret_terminal_artifact_with_effect_handler_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_structural_boolean_fields_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        &[],
        handler,
    )
}

/// Execute with exact target-neutral values for direct Boolean fields of
/// structural entry arguments. Field IDs are terminal semantic identities;
/// this input never exposes or assumes native layout.
pub fn interpret_terminal_artifact_with_structural_boolean_fields_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    structural_boolean_fields: &[TerminalStructuralBooleanFieldValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_structural_runtime_values_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        structural_boolean_fields,
        &[],
        handler,
    )
}

/// Execute with exact initial values for direct primitive structural roots.
/// The returned measurement retains their final values after all internal
/// calls have completed. This is target-neutral logical storage, not a native
/// address or layout contract.
pub fn interpret_terminal_artifact_with_structural_primitive_values_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_with_structural_runtime_values_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        &[],
        structural_primitive_values,
        handler,
    )
}

#[allow(clippy::too_many_arguments)]
fn interpret_terminal_artifact_with_structural_runtime_values_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_arguments: &[TerminalStructuralValue],
    structural_boolean_fields: &[TerminalStructuralBooleanFieldValue],
    structural_primitive_values: &[TerminalStructuralPrimitiveValue],
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut execution = TerminalExecution::start_artifact_with_structural_runtime_values(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_arguments,
        structural_boolean_fields,
        structural_primitive_values,
    )?;
    let mut meter = TerminalFuelMeter::unbounded();
    let value = match execution
        .resume_with_effect_handler(&mut meter, handler)
        .map_err(TerminalArtifactInterpretError::Execution)?
    {
        TerminalExecutionStatus::Complete(value) => value,
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Fuel(FuelMeterError::Exhausted(exhaustion)),
            ));
        }
        TerminalExecutionStatus::Crashed(crash) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::Crash(crash),
            ));
        }
    };
    let structural_primitive_values = execution.final_structural_primitive_values();
    Ok(MeasuredTerminalExecution {
        value,
        usage: meter.into_usage(),
        effects: execution.effects,
        structural_primitive_values,
    })
}

/// Decode, verify, and execute canonical terminal-Psi semantic/proof artifact
/// sections, returning only their semantic result.
pub fn interpret_terminal_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(semantic_bytes, proof_bytes, profile, arguments)
        .map(MeasuredTerminalExecution::into_value)
}

/// Decode and verify an artifact, then admit only selections that exactly name
/// rows in its canonical provider catalog.
pub fn admit_provider_installation_from_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    // The proof section must be sealed for this exact module: a section
    // naming another subject, or a bare unsealed bundle, rejects here rather
    // than reaching verification.
    let proof = terminal_codec::decode_proof_section_for(&module, proof_bytes)
        .map_err(ProviderInstallationError::ProofDecode)?;
    let verified = terminal_verifier::verify_module(&module, &proof, profile)
        .map_err(ProviderInstallationError::Verification)?;
    let mut installed = BTreeMap::new();
    for selection in selections {
        if selection.provider_identity.is_empty()
            || installed
                .insert(selection.boundary, selection.candidate)
                .is_some()
            || !verified
                .module()
                .provider_candidates
                .iter()
                .any(|candidate| {
                    candidate.boundary == selection.boundary
                        && candidate.provider_identity == selection.provider_identity
                        && candidate.candidate == selection.candidate
                })
        {
            return Err(ProviderInstallationError::UnknownOrDuplicateSelection {
                boundary: selection.boundary,
                candidate: selection.candidate,
            });
        }
    }
    Ok(AdmittedProviderInstallation {
        terminal_psi: terminal_codec::terminal_psi_identity(verified.module())
            .map_err(ProviderInstallationError::SemanticDecode)?,
        installed,
    })
}
