//! Public execution service for argument-taking build machines.
//!
//! The caller owns target selection and decides which already-validated grant
//! applies. Psi owns the target-neutral interpreter entry and the distinction
//! between a pure invocation and an explicitly granted one.

use psi_checked_interpreter::{BuildTimeValue, MeasuredBuildMachineEvaluation};
use psi_typed_trees::TypedTrees;

pub use psi_checked_interpreter::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits,
    FilesystemAccess as BuildMachineFilesystemAccess,
    FilesystemGrantRoot as BuildMachineFilesystemGrantRoot,
    FilesystemGrantRootIdentity as BuildMachineFilesystemGrantRootIdentity,
    FilesystemMetadataLayout as BuildMachineFilesystemMetadataLayout,
    FilesystemSponsor as BuildMachineFilesystemSponsor, FsGrants as BuildMachineFilesystemGrants,
};

/// Explicit execution authority supplied by build orchestration.
///
/// This service does not infer grants from a machine's reach. Callers must
/// validate that reach first and select the corresponding mode deliberately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildMachineExecutionMode {
    /// Execute through the effect-free build-time entry.
    Pure,
    /// Execute through the granted build entry with the supplied filesystem
    /// realization. Other granted host services remain governed by that
    /// entry's existing checked-interpreter contract.
    Granted {
        filesystem: BuildMachineFilesystemAccess,
        filesystem_metadata_layout: BuildMachineFilesystemMetadataLayout,
    },
}

#[derive(Debug)]
pub enum BuildMachineEvaluationError {
    Pure(String),
    Granted(psi_checked_interpreter::BuildMachineEvaluationFailure),
}

impl BuildMachineEvaluationError {
    pub fn observations(&self) -> Option<&psi_checked_interpreter::EvaluationObservations> {
        match self {
            Self::Pure(_) => None,
            Self::Granted(failure) => failure.observations(),
        }
    }
}

impl std::fmt::Display for BuildMachineEvaluationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pure(diagnostic) => formatter.write_str(diagnostic),
            Self::Granted(failure) => std::fmt::Display::fmt(failure, formatter),
        }
    }
}

impl std::error::Error for BuildMachineEvaluationError {}

/// A private, target-neutral program prepared for build-machine evaluation.
///
/// Build machines run before ordinary checked lowering, but static machine
/// parameters must observe the same complete specialization that checked
/// runtime lowering uses. Psi owns that sequencing and keeps the caller's
/// typed tree unchanged so later checking can retain template and
/// specialization-contract evidence.
pub struct PreparedBuildMachineProgram {
    typed: TypedTrees,
}

impl PreparedBuildMachineProgram {
    pub fn prepare(program: &TypedTrees) -> Result<Self, Vec<psi_diagnostics::Diagnostic>> {
        let mut typed = program.clone();
        psi_typed_trees_to_checked_trees::specialize_static_machine_calls(&mut typed)?;
        Ok(Self { typed })
    }

    pub fn typed(&self) -> &TypedTrees {
        &self.typed
    }
}

/// Evaluate one augmenting build machine and return its final argument values
/// together with deterministic evaluator usage and distinct host observations.
pub fn evaluate_build_machine_arguments_measured(
    program: &PreparedBuildMachineProgram,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationError> {
    match mode {
        BuildMachineExecutionMode::Pure => {
            psi_checked_interpreter::evaluate_build_time_machine_arguments_measured(
                program.typed(),
                machine_name,
                arguments,
            )
            .map(MeasuredBuildMachineEvaluation::hermetic)
            .map_err(BuildMachineEvaluationError::Pure)
        }
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout,
        } => psi_checked_interpreter::evaluate_build_machine_with_filesystem_measured(
            program.typed(),
            machine_name,
            arguments,
            psi_checked_interpreter::InterpretOptions {
                filesystem,
                filesystem_metadata_layout,
            },
        )
        .map_err(BuildMachineEvaluationError::Granted),
    }
}

/// Sponsored form of [`evaluate_build_machine_arguments_measured`]. All
/// invocations using clones of `sponsor` charge one aggregate deterministic
/// fuel account.
pub fn evaluate_build_machine_arguments_measured_with_sponsor(
    program: &PreparedBuildMachineProgram,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationError> {
    match mode {
        BuildMachineExecutionMode::Pure => {
            psi_checked_interpreter::evaluate_build_time_machine_arguments_measured_with_sponsor(
                program.typed(),
                machine_name,
                arguments,
                sponsor,
            )
            .map(MeasuredBuildMachineEvaluation::hermetic)
            .map_err(BuildMachineEvaluationError::Pure)
        }
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout,
        } => psi_checked_interpreter::evaluate_build_machine_with_filesystem_measured_with_sponsor(
            program.typed(),
            machine_name,
            arguments,
            psi_checked_interpreter::InterpretOptions {
                filesystem,
                filesystem_metadata_layout,
            },
            sponsor,
        )
        .map_err(BuildMachineEvaluationError::Granted),
    }
}
