//! Public execution service for argument-taking build machines.
//!
//! The caller owns target selection and decides which already-validated grant
//! applies. Psi owns the target-neutral interpreter entry and the distinction
//! between a pure invocation and an explicitly granted one.

use psi_checked_interpreter::{BuildTimeValue, MeasuredEvaluation};
use psi_typed_trees::TypedTrees;

pub use psi_checked_interpreter::{
    FilesystemAccess as BuildMachineFilesystemAccess, FsGrants as BuildMachineFilesystemGrants,
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
    },
}

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
/// together with deterministic evaluator usage.
pub fn evaluate_build_machine_arguments_measured(
    program: &PreparedBuildMachineProgram,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
) -> Result<MeasuredEvaluation<Vec<BuildTimeValue>>, String> {
    match mode {
        BuildMachineExecutionMode::Pure => {
            psi_checked_interpreter::evaluate_build_time_machine_arguments_measured(
                program.typed(),
                machine_name,
                arguments,
            )
        }
        BuildMachineExecutionMode::Granted { filesystem } => {
            psi_checked_interpreter::evaluate_build_machine_with_filesystem_measured(
                program.typed(),
                machine_name,
                arguments,
                psi_checked_interpreter::InterpretOptions { filesystem },
            )
        }
    }
}
