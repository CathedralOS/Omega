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
//! 3. construct — `TerminalProgram::load` (in `program`) admits the verified
//!    module into shared immutable form and `start_execution` binds the scalar
//!    and [`TerminalStructuralInputs`] before committing any operation;
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
    AcceptTerminalEffects, AdmittedAssumedBinding, AdmittedProviderInstallation,
    AssumedBindingSelection, ProviderInstallationSelection, TerminalEffectHandler,
};
use crate::errors::{
    AssumedBindingRejection, ProviderInstallationError, TerminalArtifactInterpretError,
    TerminalInterpretError,
};
use crate::execution::TerminalExecution;
use crate::results::{MeasuredTerminalExecution, TerminalExecutionResult, TerminalExecutionStatus};
use crate::structural_inputs::TerminalStructuralInputs;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::collections::{BTreeMap, BTreeSet};
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
    admit_provider_installation_with_assumed_bindings_from_artifact(
        semantic_bytes,
        proof_bytes,
        profile,
        selections,
        &[],
    )
}

/// Decode and verify an artifact, then admit its provider selections and the
/// consuming root's assumed binding contract relationships as one
/// installation bound to that exact artifact identity.
///
/// `assumptions` are the runtime-policy side of
/// `Build::assume_binding_contract<Required::operation, Supplied::operation>(builder)`
/// (wiki/spec/build/assumed_binding_contracts.md): only the consuming runtime
/// root may authorize them, so they arrive as sponsor input beside the
/// checked selections, never from artifact bytes — a dependency cannot grant
/// the assumption to itself. Admission replays each link against the
/// verified catalog. An unknown requirement on either side, a competing
/// checked selection or second assumption for the required requirement, a
/// supply chain which reaches no selected implementation, and a cyclic
/// binding alias each reject the whole installation.
///
/// An admitted assumption is retained on the installation as conditional
/// evidence — the artifact's guarantees hold under the recorded assumption,
/// whose falsity invalidates them without producing any unconditional claim.
/// It mints no `provider_candidates` row and no checked `installed`
/// redirect: the relationship applies at the named crossing only, so
/// ordinary boundary dispatch and direct calls of either requirement keep
/// their own evidence custody.
pub fn admit_provider_installation_with_assumed_bindings_from_artifact(
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    profile: &proof_admission::AdmissionProfile,
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    let _ = (proof_bytes, profile);
    let verified = terminal_verifier::validate_module(&module)
        .map_err(ProviderInstallationError::Validation)?;
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
    let assumed_bindings = admit_assumed_bindings(verified.module(), &installed, assumptions)?;
    Ok(AdmittedProviderInstallation {
        terminal_psi: terminal_codec::terminal_psi_identity(verified.module())
            .map_err(ProviderInstallationError::SemanticDecode)?,
        installed,
        assumed_bindings,
    })
}

/// Replay each root-authorized assumed binding against the verified catalog
/// and the admitted checked selections.
///
/// Every returned row carries the actual implementation its assumed link
/// invokes: the supply side resolves through its checked selection or
/// through nested assumed links — retaining each traversed requirement in
/// `supply_path` — and never through artifact bytes alone.
fn admit_assumed_bindings(
    module: &terminal_psi::TerminalModule,
    installed: &BTreeMap<BoundaryMachineId, MachineId>,
    assumptions: &[AssumedBindingSelection],
) -> Result<Vec<AdmittedAssumedBinding>, ProviderInstallationError> {
    let boundary_ids: BTreeSet<BoundaryMachineId> = module
        .boundary_machines
        .iter()
        .map(|boundary| boundary.id)
        .collect();
    let mut admitted = Vec::new();
    for assumption in assumptions {
        let reject = |reason| ProviderInstallationError::AssumedBindingRejected {
            required: assumption.required,
            supplied_requirement: assumption.supplied_requirement,
            reason,
        };
        if !boundary_ids.contains(&assumption.required) {
            return Err(reject(AssumedBindingRejection::UnknownRequirement));
        }
        // Competing explicit checked and assumed supplies for the same
        // operation reject rather than silently preferring the assumption.
        if installed.contains_key(&assumption.required) {
            return Err(reject(AssumedBindingRejection::CompetingCheckedSupply));
        }
        // Every required member still needs exactly one supply.
        if admitted
            .iter()
            .any(|binding: &AdmittedAssumedBinding| binding.required == assumption.required)
        {
            return Err(reject(AssumedBindingRejection::CompetingAssumption));
        }
        if !boundary_ids.contains(&assumption.supplied_requirement) {
            return Err(reject(AssumedBindingRejection::UnknownSuppliedRequirement));
        }
        // Follow the supply side's links to the actual selected
        // implementation, retaining every traversed assumption. Cyclic
        // binding aliases reject rather than resolving through themselves.
        let mut visiting = BTreeSet::from([assumption.required]);
        let mut supply_path = Vec::new();
        let mut requirement = assumption.supplied_requirement;
        let implementation = loop {
            if let Some(candidate) = installed.get(&requirement) {
                break *candidate;
            }
            if !visiting.insert(requirement) {
                return Err(reject(AssumedBindingRejection::CyclicSupply));
            }
            let Some(next) = assumptions.iter().find(|link| link.required == requirement) else {
                return Err(reject(AssumedBindingRejection::UnselectedSupply));
            };
            supply_path.push(requirement);
            requirement = next.supplied_requirement;
        };
        admitted.push(AdmittedAssumedBinding {
            required: assumption.required,
            supplied_requirement: assumption.supplied_requirement,
            implementation,
            supply_path,
        });
    }
    Ok(admitted)
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
        // One admission path serves both public surfaces: `load` decodes the
        // subject-sealed sections and verifies under the interpretation
        // profile; `start_execution` binds this record's inputs.
        crate::program::TerminalProgram::load(semantic_bytes, proof_bytes, profile)?
            .start_execution(scalar_arguments, structural_inputs, None)
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
        let _ = (proof_bytes, profile);
        terminal_verifier::validate_module(&module)
            .map_err(TerminalArtifactInterpretError::Validation)?;
        let program = crate::program::TerminalProgram::from_admitted(
            crate::program::AdmittedTerminalProgram::admit(module)
                .map_err(TerminalArtifactInterpretError::Execution)?,
        );
        program.start_execution(scalar_arguments, structural_inputs, Some(installation))
    }
}
