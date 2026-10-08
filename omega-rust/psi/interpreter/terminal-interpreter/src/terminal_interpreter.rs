//! Fuel-bounded reference execution for Terminal-Psi artifacts.
//!
//! The public entries accept canonical semantic bytes; no source or checked-tree
//! representation crosses this boundary. Every entry runs the same sequence:
//!
//! 1. decode — `terminal_codec::decode_module` reconstructs the module;
//! 2. construct — `TerminalProgram::load` (in `program`) retains the module in
//!    shared immutable form and `start_execution` binds the scalar
//!    and [`TerminalStructuralInputs`] before committing any operation;
//! 3. run — [`TerminalExecution::resume`] charges fuel and dispatches every
//!    operation and terminator until the artifact completes, crashes or the
//!    sponsor's fuel is exhausted.
//!
//! [`interpret_serialized_terminal_artifact_measured`] starts from one
//! portable envelope, [`interpret_terminal_artifact_measured`] from its
//! semantic section, and [`interpret_terminal_artifact`] returns
//! only the semantic result under the accepting effect handler. The two
//! `TerminalExecution` starts below are steps 1 and 2 for a caller that drives
//! `resume` itself. Provider admission borrows an already retained artifact
//! through [`admit_provider_installation`], or decodes external semantic bytes
//! through [`admit_provider_installation_from_artifact`]. Both check the same
//! selection policy against the catalog instead of running.
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

/// Decode one complete portable Terminal-Psi envelope and execute its module
/// with a fresh effect-policy
/// input supplied by the receiver. The envelope contains no checked-tree or
/// build-process object.
pub fn interpret_serialized_terminal_artifact_measured(
    artifact_bytes: &[u8],
    scalar_arguments: &[TerminalScalarValue],
    structural_inputs: TerminalStructuralInputs<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let program = crate::program::TerminalProgram::load_envelope(artifact_bytes)?;
    let execution = program.start_execution(scalar_arguments, structural_inputs, None)?;
    execute_measured(execution, handler)
}

/// Decode and run one artifact to completion under `handler`,
/// measuring its fuel; every structural input arrives in one record.
pub fn interpret_terminal_artifact_measured(
    semantic_bytes: &[u8],
    scalar_arguments: &[TerminalScalarValue],
    structural_inputs: TerminalStructuralInputs<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
    let execution =
        TerminalExecution::start_artifact(semantic_bytes, scalar_arguments, structural_inputs)?;
    execute_measured(execution, handler)
}

fn execute_measured(
    mut execution: TerminalExecution<'_>,
    handler: &mut impl TerminalEffectHandler,
) -> Result<MeasuredTerminalExecution, TerminalArtifactInterpretError> {
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
        TerminalExecutionStatus::ExternalCompletion(completion) => {
            return Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::ExternalCompletion(completion),
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

/// Decode and execute canonical Terminal-Psi semantic bytes, returning only
/// their semantic result.
pub fn interpret_terminal_artifact(
    semantic_bytes: &[u8],
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    interpret_terminal_artifact_measured(
        semantic_bytes,
        arguments,
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .map(MeasuredTerminalExecution::into_value)
}

/// Decode a canonical artifact, then admit only selections that exactly name
/// rows in its canonical provider catalog.
pub fn admit_provider_installation_from_artifact(
    semantic_bytes: &[u8],
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    admit_provider_installation_with_assumed_bindings_from_artifact(semantic_bytes, selections, &[])
}

/// Admit receiving-host selections against a retained canonical artifact.
/// The immutable view preserves the module/identity relationship; selection
/// and assumption checks are identical to external byte admission. No module
/// decoding or identity reconstruction is needed within the same invocation.
pub fn admit_provider_installation(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    admit_provider_selections(
        artifact.module(),
        artifact.identity(),
        selections,
        assumptions,
    )
}

/// Decode a canonical artifact, then admit its provider selections and the
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
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    let identity = terminal_codec::terminal_psi_identity(&module)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    admit_provider_selections(&module, identity, selections, assumptions)
}

fn admit_provider_selections(
    module: &terminal_psi::TerminalModule,
    identity: terminal_psi::TerminalPsiIdentity,
    selections: &[ProviderInstallationSelection],
    assumptions: &[AssumedBindingSelection],
) -> Result<AdmittedProviderInstallation, ProviderInstallationError> {
    let mut installed = BTreeMap::new();
    for selection in selections {
        if selection.provider_identity.is_empty()
            || installed
                .insert(selection.boundary, selection.candidate)
                .is_some()
            || !module.provider_candidates.iter().any(|candidate| {
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
    let assumed_bindings = admit_assumed_bindings(module, &installed, assumptions)?;
    Ok(AdmittedProviderInstallation {
        terminal_psi: identity,
        supplies: installed
            .into_iter()
            .map(|(boundary, machine)| {
                (
                    boundary,
                    crate::effects::AdmittedBoundarySupply::CheckedProvider(machine),
                )
            })
            .collect(),
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

impl TerminalExecution<'_> {
    /// Decode one artifact, then bind its scalar arguments and structural
    /// inputs before committing any operation or custody transfer.
    pub fn start_artifact(
        semantic_bytes: &[u8],
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        // Both public surfaces use the same immutable program and input binding.
        crate::program::TerminalProgram::load(semantic_bytes)?.start_execution(
            scalar_arguments,
            structural_inputs,
            None,
        )
    }

    /// Begin execution with one explicit provider installation previously
    /// admitted against these exact semantic/proof sections. Fixed-array inputs
    /// supply exact initialized backing; an installed provider grants no storage.
    pub fn start_installed_artifact(
        semantic_bytes: &[u8],
        scalar_arguments: &[TerminalScalarValue],
        structural_inputs: TerminalStructuralInputs<'_>,
        installation: &AdmittedProviderInstallation,
    ) -> Result<Self, TerminalArtifactInterpretError> {
        let program = crate::program::TerminalProgram::load(semantic_bytes)?;
        program.start_execution(scalar_arguments, structural_inputs, Some(installation))
    }
}
