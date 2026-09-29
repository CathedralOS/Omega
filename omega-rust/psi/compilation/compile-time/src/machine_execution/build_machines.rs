//! Public execution service for argument-taking build machines.
//!
//! The caller owns target selection and decides which already-validated grant
//! applies. Psi owns the target-neutral interpreter entry and the distinction
//! between a pure invocation and an explicitly granted one.

use checked_interpreter::{BuildTimeValue, MeasuredBuildMachineEvaluation};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub use checked_interpreter::{
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
    Granted(checked_interpreter::BuildMachineEvaluationFailure),
}

impl BuildMachineEvaluationError {
    pub fn observations(&self) -> Option<&checked_interpreter::EvaluationObservations> {
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
    program: PreparedBuildMachineBody,
    identity: Arc<PreparedBuildMachineProgramIdentity>,
}

enum PreparedBuildMachineBody {
    Typed(Box<TypedTrees>),
    Checked(Box<checked_trees::CheckedTrees>),
}

/// Exact build-machine entry owned by one prepared program.
///
/// A raw [`SymbolHandle`] is local to an arena and can have the same numeric
/// representation in another program. This token couples that exact handle to
/// the preparation that admitted it, so execution can reject cross-program
/// substitution before entering the interpreter. Its reachable static applications
/// are closed; operational contracts and host grants remain caller obligations.
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
    /// Transfer the checked program and its execution facts into one immutable
    /// identity. No specialization or rebinding follows admission, so retained
    /// facts keep naming the exact arena the interpreter consumes.
    /// Host grants and the exact entry's operational contract remain caller duties.
    pub fn from_checked(program: checked_trees::CheckedTrees) -> Self {
        Self {
            program: PreparedBuildMachineBody::Checked(Box::new(program)),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        }
    }

    pub fn prepare(program: &TypedTrees) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let mut typed = program.clone();
        // Preparation may precede endpoint evaluation. Preserve those exact
        // obligations on the private copy: specialization leaves a tuple with
        // pending type bounds unreplaced, so it cannot execute before a later
        // preparation validates its fully evaluated arguments.
        crate::const_evaluation::range_endpoints::defer_pending_endpoint_calls(&mut typed)?;
        typed_trees_to_checked_trees::specialize_static_machine_calls(&mut typed)?;
        Ok(Self {
            program: PreparedBuildMachineBody::Typed(Box::new(typed)),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        })
    }

    pub fn typed(&self) -> &TypedTrees {
        match &self.program {
            PreparedBuildMachineBody::Typed(typed) => typed,
            PreparedBuildMachineBody::Checked(checked) => &checked.typed,
        }
    }

    fn interpreter_program(&self) -> checked_interpreter::BuildMachineProgram<'_> {
        match &self.program {
            PreparedBuildMachineBody::Typed(typed) => {
                checked_interpreter::BuildMachineProgram::Typed(typed)
            }
            PreparedBuildMachineBody::Checked(checked) => {
                checked_interpreter::BuildMachineProgram::Checked(checked)
            }
        }
    }

    /// Bind one exact machine with closed static applications to this immutable
    /// prepared program. The lookup never falls back to a matching name. No
    /// execution mode or host grant can admit an unfinished specialization.
    pub fn entry(&self, symbol: SymbolHandle) -> Result<PreparedBuildMachineEntry, String> {
        if !self
            .typed()
            .machines()
            .iter()
            .any(|machine| machine.symbol == symbol)
        {
            return Err(format!(
                "prepared build program contains no machine with exact symbol {symbol:?}"
            ));
        }
        if self.typed().machines().iter().any(|machine| {
            machine.structural_type_equations_pending || !machine.type_parameters.is_empty()
        }) {
            super::admission::BuildTimeAdmissionPlan::infer(self.typed(), None)
                .require_closed_static_applications(self.typed(), symbol)?;
        }
        Ok(PreparedBuildMachineEntry {
            program_identity: Arc::clone(&self.identity),
            symbol,
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
        // Entry construction checked membership and static closure. The prepared program
        // is immutable, so matching its identity preserves that check without
        // searching the machine roster again at execution.
        Ok(entry.symbol)
    }
}

/// One measured build-machine invocation: the exact prepared entry, its arguments, the
/// pure or granted execution mode, and the sponsor that pays for its custody.
pub struct BuildMachineInvocation<'a> {
    pub entry: &'a PreparedBuildMachineEntry,
    pub arguments: Vec<BuildTimeValue>,
    pub mode: BuildMachineExecutionMode,
    pub sponsor: Option<&'a BuildEvaluationSponsor>,
    pub product_entry_compatibility: Option<&'a dyn checked_interpreter::ProductEntryCompatibility>,
}

/// Evaluate one build machine and measure its evaluation. All build-machine
/// evaluations enter here: the mode selects the pure or filesystem-granted
/// interpreter and the sponsor, when present, is charged for result custody.
pub fn evaluate_build_machine_measured(
    program: &PreparedBuildMachineProgram,
    invocation: BuildMachineInvocation<'_>,
) -> Result<MeasuredBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationError> {
    let symbol = program.validate_entry(invocation.entry)?;
    let request = checked_interpreter::BuildMachineEvaluationRequest {
        entry: checked_interpreter::BuildMachineEntry::Symbol(symbol),
        arguments: invocation.arguments,
        operators: &[],
        sponsor: invocation.sponsor,
        product_entry_compatibility: invocation.product_entry_compatibility,
    };
    match invocation.mode {
        BuildMachineExecutionMode::Pure => checked_interpreter::evaluate_build_machine_arguments(
            program.interpreter_program(),
            request,
        )
        .map_err(BuildMachineEvaluationError::Pure),
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout,
        } => checked_interpreter::evaluate_granted_build_machine_arguments(
            program.interpreter_program(),
            request,
            checked_interpreter::InterpretOptions {
                filesystem,
                filesystem_metadata_layout,
                ..Default::default()
            },
        )
        .map_err(BuildMachineEvaluationError::Granted),
    }
}
