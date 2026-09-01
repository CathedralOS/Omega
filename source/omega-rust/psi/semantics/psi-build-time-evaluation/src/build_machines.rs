//! Public execution service for argument-taking build machines.
//!
//! The caller owns target selection and decides which already-validated grant
//! applies. Psi owns the target-neutral interpreter entry and the distinction
//! between a pure invocation and an explicitly granted one.

use psi_checked_interpreter::{BuildTimeValue, MeasuredBuildMachineEvaluation};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use std::sync::Arc;

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
    Entry(String),
    Pure(String),
    Granted(psi_checked_interpreter::BuildMachineEvaluationFailure),
}

impl BuildMachineEvaluationError {
    pub fn observations(&self) -> Option<&psi_checked_interpreter::EvaluationObservations> {
        match self {
            Self::Entry(_) => None,
            Self::Pure(_) => None,
            Self::Granted(failure) => failure.observations(),
        }
    }
}

impl std::fmt::Display for BuildMachineEvaluationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entry(diagnostic) => formatter.write_str(diagnostic),
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
    identity: Arc<PreparedBuildMachineProgramIdentity>,
}

/// Exact build-machine entry owned by one prepared program.
///
/// A raw [`SymbolHandle`] is local to an arena and can have the same numeric
/// representation in another program. This token couples that exact handle to
/// the preparation that admitted it, so execution can reject cross-program
/// substitution before entering the interpreter.
#[derive(Clone)]
pub struct PreparedBuildMachineEntry {
    program_identity: Arc<PreparedBuildMachineProgramIdentity>,
    symbol: SymbolHandle,
}

struct PreparedBuildMachineProgramIdentity {
    // Keep the allocation non-zero-sized so pointer identity is unambiguous.
    _private: u8,
}

impl PreparedBuildMachineEntry {
    pub const fn symbol(&self) -> SymbolHandle {
        self.symbol
    }
}

impl PreparedBuildMachineProgram {
    pub fn prepare(program: &TypedTrees) -> Result<Self, Vec<psi_diagnostics::Diagnostic>> {
        let mut typed = program.clone();
        psi_typed_trees_to_checked_trees::specialize_static_machine_calls(&mut typed)?;
        Ok(Self {
            typed,
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        })
    }

    pub fn typed(&self) -> &TypedTrees {
        &self.typed
    }

    /// Bind one exact machine declaration to this prepared program. The
    /// lookup is symbol-only and never falls back to a matching name.
    pub fn entry(&self, symbol: SymbolHandle) -> Result<PreparedBuildMachineEntry, String> {
        self.typed
            .machines()
            .iter()
            .any(|machine| machine.symbol == symbol)
            .then(|| PreparedBuildMachineEntry {
                program_identity: Arc::clone(&self.identity),
                symbol,
            })
            .ok_or_else(|| {
                format!("prepared build program contains no machine with exact symbol {symbol:?}")
            })
    }

    fn validate_entry(
        &self,
        entry: &PreparedBuildMachineEntry,
    ) -> Result<SymbolHandle, BuildMachineEvaluationError> {
        if !Arc::ptr_eq(&self.identity, &entry.program_identity) {
            return Err(BuildMachineEvaluationError::Entry(
                "prepared build-machine entry belongs to a different prepared program".to_owned(),
            ));
        }
        self.typed
            .machines()
            .iter()
            .any(|machine| machine.symbol == entry.symbol)
            .then_some(entry.symbol)
            .ok_or_else(|| {
                BuildMachineEvaluationError::Entry(format!(
                    "prepared build-machine entry's exact symbol {:?} is absent from its program",
                    entry.symbol
                ))
            })
    }
}

/// Evaluate one exact prepared entry and return its final argument values.
///
/// This is the compiler-facing D18 path. Entry ownership is checked before
/// execution, and the checked interpreter resolves only the retained symbol.
pub fn evaluate_build_machine_entry_arguments_measured(
    program: &PreparedBuildMachineProgram,
    entry: &PreparedBuildMachineEntry,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationError> {
    let machine_symbol = program.validate_entry(entry)?;
    match mode {
        BuildMachineExecutionMode::Pure => {
            psi_checked_interpreter::evaluate_observed_build_time_machine_symbol_arguments_measured(
                program.typed(),
                machine_symbol,
                arguments,
            )
            .map_err(BuildMachineEvaluationError::Pure)
        }
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout,
        } => psi_checked_interpreter::evaluate_build_machine_symbol_with_filesystem_measured(
            program.typed(),
            machine_symbol,
            arguments,
            psi_checked_interpreter::InterpretOptions {
                filesystem,
                filesystem_metadata_layout,
                ..Default::default()
            },
        )
        .map_err(BuildMachineEvaluationError::Granted),
    }
}

/// Sponsored form of [`evaluate_build_machine_entry_arguments_measured`].
pub fn evaluate_build_machine_entry_arguments_measured_with_sponsor(
    program: &PreparedBuildMachineProgram,
    entry: &PreparedBuildMachineEntry,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
    sponsor: &BuildEvaluationSponsor,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationError> {
    let machine_symbol = program.validate_entry(entry)?;
    match mode {
        BuildMachineExecutionMode::Pure => {
            psi_checked_interpreter::evaluate_observed_build_time_machine_symbol_arguments_measured_with_sponsor(
                program.typed(),
                machine_symbol,
                arguments,
                sponsor,
            )
            .map_err(BuildMachineEvaluationError::Pure)
        }
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout,
        } => psi_checked_interpreter::evaluate_build_machine_symbol_with_filesystem_measured_with_sponsor(
            program.typed(),
            machine_symbol,
            arguments,
            psi_checked_interpreter::InterpretOptions {
                filesystem,
                filesystem_metadata_layout,
                ..Default::default()
            },
            sponsor,
        )
        .map_err(BuildMachineEvaluationError::Granted),
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
            psi_checked_interpreter::evaluate_observed_build_time_machine_arguments_measured(
                program.typed(),
                machine_name,
                arguments,
            )
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
                ..Default::default()
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
            psi_checked_interpreter::evaluate_observed_build_time_machine_arguments_measured_with_sponsor(
                program.typed(),
                machine_name,
                arguments,
                sponsor,
            )
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
                ..Default::default()
            },
            sponsor,
        )
        .map_err(BuildMachineEvaluationError::Granted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_checked_interpreter::{
        FilesystemGrantRootIdentity, FilesystemInputOutputTreeReplayRecord,
        FilesystemOutputChangeFileOwnerReplayRecord, FilesystemOutputFileOperationReplayRecord,
        FilesystemOutputFileReplayRecord, FilesystemOutputTreeEntryReplayRecord, FilesystemReplay,
    };

    #[test]
    fn granted_mode_preserves_exact_output_ownership_replay_authority() {
        let operations = vec![
            FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                FilesystemOutputChangeFileOwnerReplayRecord::new(-1, -1, 0, 0).unwrap(),
            ),
            FilesystemOutputFileOperationReplayRecord::ChangeFileOwner(
                FilesystemOutputChangeFileOwnerReplayRecord::new(0, 0, -1, 1).unwrap(),
            ),
        ];
        let output = FilesystemOutputFileReplayRecord::with_operations(
            FilesystemGrantRootIdentity::new(2).unwrap(),
            b"owned.bin".to_vec(),
            7,
            0,
            operations,
            1,
        )
        .unwrap();
        let replay = FilesystemReplay::from_input_output_tree_record(
            FilesystemInputOutputTreeReplayRecord::output_only(
                vec![FilesystemOutputTreeEntryReplayRecord::File(output)],
                Vec::new(),
            )
            .unwrap(),
        )
        .unwrap();
        let mode = BuildMachineExecutionMode::Granted {
            filesystem: BuildMachineFilesystemAccess::ReplayFilesystem(replay),
            filesystem_metadata_layout: Default::default(),
        };

        let BuildMachineExecutionMode::Granted {
            filesystem: BuildMachineFilesystemAccess::ReplayFilesystem(replay),
            ..
        } = mode
        else {
            panic!("granted mode changed replay authority")
        };
        assert_eq!(
            replay
                .attempts()
                .iter()
                .map(|attempt| attempt.operation_tag())
                .collect::<Vec<_>>(),
            vec![1, 49, 49, 8]
        );
        assert_eq!(replay.attempts().last().unwrap().post_error(), Some(1));
    }
}
