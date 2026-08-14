//! Public execution service for argument-taking build machines.
//!
//! The caller owns target selection and decides which already-validated grant
//! applies. Psi owns the target-neutral interpreter entry and the distinction
//! between a pure invocation and an explicitly granted one.

use psi_checked_interpreter::{BuildTimeValue, MeasuredEvaluation};
use psi_typed_trees::TypedTrees;

pub use psi_checked_interpreter::FilesystemAccess as BuildMachineFilesystemAccess;

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

/// Evaluate one augmenting build machine and return its final argument values
/// together with deterministic evaluator usage.
pub fn evaluate_build_machine_arguments_measured(
    program: &TypedTrees,
    machine_name: &str,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
) -> Result<MeasuredEvaluation<Vec<BuildTimeValue>>, String> {
    match mode {
        BuildMachineExecutionMode::Pure => {
            psi_checked_interpreter::evaluate_build_time_machine_arguments_measured(
                program,
                machine_name,
                arguments,
            )
        }
        BuildMachineExecutionMode::Granted { filesystem } => {
            psi_checked_interpreter::evaluate_build_machine_with_filesystem_measured(
                program,
                machine_name,
                arguments,
                psi_checked_interpreter::InterpretOptions { filesystem },
            )
        }
    }
}
