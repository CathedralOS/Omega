//! Fuel-bounded reference execution for verified terminal-Psi artifacts:
//! where interpretation starts.
//!
//! The public entries accept only canonical semantic/proof bytes and an
//! admission profile; no source or checked-tree representation crosses this
//! boundary. Every entry runs the same sequence:
//!
//! 1. decode — `terminal_codec::decode_module` reconstructs the module and
//!    `decode_proof_section_for` accepts only the proof section sealed for
//!    that exact module;
//! 2. verify — `terminal_verifier` checks the module against the profile
//!    before any runtime state exists;
//! 3. construct — `TerminalExecution::start_verified_module` (in
//!    `execution`) admits the verified machines and binds the scalar and
//!    [`TerminalStructuralInputs`] before committing any operation;
//! 4. run — [`TerminalExecution::resume`] charges fuel and dispatches every
//!    operation and terminator until the artifact completes, crashes or the
//!    sponsor's fuel is exhausted.
//!
//! [`interpret_serialized_terminal_artifact_measured`] starts from one
//! portable envelope, [`interpret_terminal_artifact_measured`] from its
//! semantic and proof sections, and [`interpret_terminal_artifact`] returns
//! only the semantic result under the accepting effect handler. The two
//! `TerminalExecution` starts below are steps 1 to 3 for a caller that drives
//! `resume` itself; [`admit_provider_installation_from_artifact`] decodes and
//! verifies the same way and admits provider selections against the
//! verified catalog instead of running.
//!
//! The domains beside this file: `execution` owns the live state and the
//! interpreter loop; `structural_inputs` the host-supplied entry inputs and
//! their binding; `custody`, `reference`, `primitive_storage`, `record` and
//! `byte_sequences` the ownership and storage the loop mutates;
//! `scalar_operations`, `structural_operations`, `calls` and `terminators`
//! the operations the loop dispatches; `effects` what the loop hands to the
//! host; `values`, `results` and `errors` what crosses the boundary; and
//! `semantic_value_comparison` the trace-value comparison used by
//! differential checks.

use crate::effects::{
    AcceptTerminalEffects, AdmittedProviderInstallation, ProviderInstallationSelection,
    TerminalEffectHandler,
};
use crate::errors::{
    ProviderInstallationError, TerminalArtifactInterpretError, TerminalInterpretError,
};
use crate::execution::TerminalExecution;
use crate::results::{MeasuredTerminalExecution, TerminalExecutionResult, TerminalExecutionStatus};
use crate::structural_inputs::TerminalStructuralInputs;
use crate::structural_inputs::scalar_fields::TerminalStructuralScalarFieldValue;
use crate::values::TerminalScalarValue;
use std::collections::BTreeMap;
use terminal_fuel::{FuelMeterError, TerminalFuelMeter};

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

impl TerminalExecution {
    /// Decode, verify and start one artifact with its scalar arguments and
    /// every structural input in one record. This is the only start entry.
    /// Decode and independently verify an artifact, then bind its initialized
    /// entry contents before committing any operation or custody transfer.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        // Only the subject-sealed proof section enters interpretation: the
        // seal must name this module's reconstructed identity, so a proof
        // produced for another source/model/profile cannot be replayed here.
        let proof = terminal_codec::decode_proof_section_for(&module, proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let _verified =
            terminal_verifier::verify_module_for_interpretation(&module, &proof, profile)
                .map_err(TerminalArtifactInterpretError::Verification)?;
        let converted_boolean_fields;
        let scalar_fields = if structural_inputs.boolean_fields.is_empty() {
            structural_inputs.scalar_fields
        } else {
            converted_boolean_fields = structural_inputs
                .boolean_fields
                .iter()
                .map(TerminalStructuralScalarFieldValue::from)
                .collect::<Vec<_>>();
            &converted_boolean_fields
        };
        let mut execution = Self::start_verified_module(
            module,
            scalar_arguments,
            structural_inputs.arguments,
            scalar_fields,
            structural_inputs.primitive_values,
            structural_inputs.placed_view_establishments,
            None,
        )
        .map_err(TerminalArtifactInterpretError::Execution)
        .map_err(|error| match error {
            TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index,
                    field,
                },
            ) if !structural_inputs.boolean_fields.is_empty() => {
                TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::StructuralBooleanFieldArgumentInvalid {
                        argument_index,
                        field,
                    },
                )
            }
            other => other,
        })?;
        execution
            .bind_structural_cases(structural_inputs.cases)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        if !structural_inputs.byte_arrays.is_empty() {
            execution
                .bind_byte_arrays(structural_inputs.byte_arrays)
                .map_err(TerminalArtifactInterpretError::Execution)?;
        }
        Ok(execution)
    }

    /// Begin execution with one explicit provider installation previously
    /// admitted against these exact semantic/proof sections. Fixed-array inputs
    /// supply exact initialized backing; an installed provider grants no storage.
    pub fn start_installed_artifact(
        semantic_bytes: &[u8],
        proof_bytes: &[u8],
        profile: &proof_admission::AdmissionProfile,
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: &AdmittedProviderInstallation,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(TerminalArtifactInterpretError::SemanticDecode)?;
        // Same sealed-section requirement as the other artifact starts: an
        // admitted installation is bound to this exact Terminal-Psi identity.
        let proof = terminal_codec::decode_proof_section_for(&module, proof_bytes)
            .map_err(TerminalArtifactInterpretError::ProofDecode)?;
        let _verified = terminal_verifier::verify_module(&module, &proof, profile)
            .map_err(TerminalArtifactInterpretError::Verification)?;
        let mut execution = Self::start_verified_module(
            module,
            scalar_arguments,
            structural_inputs.arguments,
            structural_inputs.scalar_fields,
            structural_inputs.primitive_values,
            structural_inputs.placed_view_establishments,
            Some(installation),
        )
        .map_err(TerminalArtifactInterpretError::Execution)?;
        if !structural_inputs.cases.is_empty() {
            execution
                .bind_structural_cases(structural_inputs.cases)
                .map_err(TerminalArtifactInterpretError::Execution)?;
        }
        execution
            .bind_byte_arrays(structural_inputs.byte_arrays)
            .map_err(TerminalArtifactInterpretError::Execution)?;
        Ok(execution)
    }
}
