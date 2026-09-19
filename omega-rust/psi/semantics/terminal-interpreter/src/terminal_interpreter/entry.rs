//! The public entries: interpreting a verified artifact under an admission
//! profile, with or without an effect handler, structural inputs and a fuel
//! measure, and admitting a provider installation from an artifact.

use crate::TerminalStructuralByteArrayValue;
use crate::TerminalStructuralCaseValue;
use crate::TerminalStructuralScalarFieldValue;
use crate::terminal_interpreter::{
    AcceptTerminalEffects, AdmittedProviderInstallation, MeasuredTerminalExecution,
    ProviderInstallationError, ProviderInstallationSelection, TerminalArtifactInterpretError,
    TerminalEffectHandler, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalPlacedViewEstablishment, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use std::collections::BTreeMap;
use terminal_fuel::{FuelMeterError, TerminalFuelMeter};

/// Exact initialized contents supplied by the embedding host for structural
/// entry arguments. All paths remain rooted in the original referents.
/// Every structural input one artifact execution starts with. The default is
/// no structural input at all; each field is bound only when it is given.
#[derive(Debug, Clone, Copy, Default)]
pub struct TerminalStructuralInputs<'input> {
    pub arguments: &'input [TerminalStructuralValue],
    pub scalar_fields: &'input [TerminalStructuralScalarFieldValue],
    /// Boolean fields are bound as scalar fields; a rejected boolean field is
    /// reported as a boolean-field argument error.
    pub boolean_fields: &'input [TerminalStructuralBooleanFieldValue],
    pub primitive_values: &'input [TerminalStructuralPrimitiveValue],
    pub cases: &'input [TerminalStructuralCaseValue],
    pub byte_arrays: &'input [TerminalStructuralByteArrayValue],
    /// Established placed-view inputs: one exact establishment per direct
    /// entry roster row lends its qualified referent backing for the
    /// invocation's duration. An entry declaring no placed-view inputs leaves
    /// this empty.
    pub placed_view_establishments: &'input [TerminalPlacedViewEstablishment],
}

/// Decode one complete portable Terminal-Psi envelope, independently verify
/// its semantic and proof sections, and execute it with a fresh effect-policy
/// input supplied by the receiver. The envelope contains no checked-tree or
/// build-process object.
pub fn interpret_serialized_terminal_artifact_measured(
    artifact_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_inputs: TerminalStructuralInputs<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(artifact_bytes)
        .map_err(TerminalArtifactInterpretError::ArtifactDecode)?;
    interpret_terminal_artifact_measured(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        profile,
        scalar_arguments,
        structural_inputs,
        handler,
    )
}

/// Decode, verify and run one artifact to completion under `handler`,
/// measuring its fuel; every structural input arrives in one record.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    scalar_arguments: &[TerminalScalarValue],
    structural_inputs: TerminalStructuralInputs<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let mut execution = TerminalExecution::start_artifact(
        semantic_bytes,
        proof_bytes,
        profile,
        scalar_arguments,
        structural_inputs,
    )?;
    let mut meter = TerminalFuelMeter::unbounded();
    let value = match execution
        .resume(&mut meter, handler)
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
    interpret_terminal_artifact_measured(
        semantic_bytes,
        proof_bytes,
        profile,
        arguments,
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
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
