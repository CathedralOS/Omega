//! Public execution service for argument-taking build machines.
//!
//! The caller owns target selection and decides which already-validated grant
//! applies. Psi owns the target-neutral interpreter entry and the distinction
//! between a pure invocation and an explicitly granted one.

use build_time_values::BuildTimeValue;
use checked_interpreter::MeasuredBuildMachineEvaluation;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::product_operand::ProductEntryCompatibility;

mod numeric_imports;
#[cfg(test)]
mod tests;

pub use checked_interpreter::{
    BuildEvaluationSponsor, BuildEvaluationSponsorLimits,
    CapturedFilesystemRoot as BuildMachineCapturedFilesystemRoot,
    FilesystemAccess as BuildMachineFilesystemAccess,
    FilesystemGrantRoot as BuildMachineFilesystemGrantRoot,
    FilesystemGrantRootIdentity as BuildMachineFilesystemGrantRootIdentity,
    FilesystemMetadataLayout as BuildMachineFilesystemMetadataLayout,
    FilesystemSponsor as BuildMachineFilesystemSponsor,
    FilesystemSponsorLimits as BuildMachineFilesystemSponsorLimits,
    FsGrants as BuildMachineFilesystemGrants,
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
/// Build machines run before ordinary checked lowering, but closed static
/// applications and numeric destinations must use the same formation as
/// runtime lowering. Psi owns that sequencing and keeps the caller's
/// typed tree unchanged so later checking can retain template and
/// specialization-contract evidence.
pub struct PreparedBuildMachineProgram {
    program: PreparedBuildMachineBody,
    identity: Arc<PreparedBuildMachineProgramIdentity>,
}

enum PreparedBuildMachineBody {
    Typed(Box<TypedTrees>),
    Checked(CheckedBuildProgram),
}

/// Immutable checked execution content, including all CheckFacts. Equality
/// compares the complete checked value, not typed trees or arena token identity.
/// It contains no executable prepared entry and no host authority.
#[derive(Clone, PartialEq, Eq)]
pub struct CheckedBuildProgram {
    checked: Arc<checked_trees::CheckedTrees>,
    /// Descriptor queries are observable Build inputs, even when their bodies
    /// are absent from the checked executable closure.
    product_declarations: Arc<TypedTrees>,
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
        let product_declarations = program
            .typed
            .product_declaration_frontier
            .clone()
            .unwrap_or_else(|| Arc::new(program.typed.as_ref().clone()));
        Self {
            program: PreparedBuildMachineBody::Checked(CheckedBuildProgram {
                checked: Arc::new(program),
                product_declarations,
            }),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        }
    }

    pub fn prepare(program: &TypedTrees) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        Self::prepare_in(
            program,
            &crate::build_time_evaluation::EvaluationDomain::all(),
        )
    }

    pub(crate) fn prepare_in(
        program: &TypedTrees,
        domain: &crate::build_time_evaluation::EvaluationDomain,
    ) -> Result<Self, Vec<diagnostics::Diagnostic>> {
        let mut typed = domain.execution_program(program);
        // Preparation may precede endpoint evaluation. Preserve those exact
        // obligations on the private copy: specialization leaves a tuple with
        // pending type bounds unreplaced, so it cannot execute before a later
        // preparation validates its fully evaluated arguments.
        crate::const_evaluation::range_endpoints::defer_pending_endpoint_calls(&mut typed)?;
        let typed = typed_trees_to_typed_trees::prepare_constant_execution(typed)?;
        Ok(Self {
            program: PreparedBuildMachineBody::Typed(Box::new(typed)),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        })
    }

    pub fn typed(&self) -> &TypedTrees {
        match &self.program {
            PreparedBuildMachineBody::Typed(typed) => typed,
            PreparedBuildMachineBody::Checked(program) => &program.checked.typed,
        }
    }

    /// Retain the complete immutable checked program, including CheckFacts.
    /// This is content only, never the preparation's executable entry token.
    pub fn checked_program(&self) -> Option<CheckedBuildProgram> {
        match &self.program {
            PreparedBuildMachineBody::Checked(program) => Some(program.clone()),
            PreparedBuildMachineBody::Typed(_) => None,
        }
    }

    /// Borrow the admitted language judgment for ordinary Terminal production.
    /// This exposes content, not host grants or an executable entry token.
    pub fn checked_trees(&self) -> Option<&checked_trees::CheckedTrees> {
        match &self.program {
            PreparedBuildMachineBody::Checked(program) => Some(&program.checked),
            PreparedBuildMachineBody::Typed(_) => None,
        }
    }

    /// Execute an admitted zero-argument constant through the common Terminal
    /// producer and instance lifecycle. Source admission and exact selected
    /// operator custody precede execution, which cannot rewrite that judgment.
    /// Only admitted numeric supplies are installed; other imports refuse.
    pub(crate) fn evaluate_checked_constant(
        &self,
        entry: &PreparedBuildMachineEntry,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
    ) -> Result<terminal_interpreter::TerminalExecutionResult, String> {
        let PreparedBuildMachineBody::Checked(program) = &self.program else {
            return Err("constant invocation requires a checked preparation".into());
        };
        let symbol = self
            .validate_entry(entry)
            .map_err(|error| error.to_string())?;
        super::selected_operators::validate_selected_operators(self.typed(), operators)?;
        let toolchain_sources = source::source_identity::toolchain_source_identities(
            program.checked.symbols.source_files(),
        )?;
        let artifact = psi_terminal_production::TerminalProductionRequest::new(
            &program.checked,
            psi_terminal_production::TerminalMachineSelection::Symbol(symbol),
            &toolchain_sources,
        )
        .produce(
            Default::default(),
            &mut psi_terminal_production::UnmeasuredProduction,
        )
        .map_err(|error| error.to_string())?
        .into_artifact()
        .map_err(|error| error.to_string())?;
        let program = terminal_interpreter::TerminalProgram::borrow(artifact.view());
        let mut instance = program
            .instantiate(terminal_interpreter::TerminalInstantiation {
                scalar_arguments: &[],
                structural_inputs: Default::default(),
                ..Default::default()
            })
            .map_err(|error| error.to_string())?;
        numeric_imports::install(&program, &mut instance, self.typed(), operators)?;
        // This is a Terminal semantic allowance, not a checked-tree step count.
        // Exhaustion refuses this constant; it never retries in another engine.
        let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(100_000);
        match instance
            .invoke(&mut meter)
            .map_err(|error| format!("constant Terminal invocation failed: {error:?}"))?
        {
            terminal_interpreter::TerminalExecutionStatus::Complete(value) => Ok(value),
            terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => Err(format!(
                "constant Terminal execution exhausted its fuel allowance after {} units",
                meter.usage().total_units(),
            )),
            terminal_interpreter::TerminalExecutionStatus::Crashed(crash) => {
                Err(format!("constant Terminal execution crashed: {crash:?}"))
            }
            terminal_interpreter::TerminalExecutionStatus::ExternalCompletion(completion) => Err(
                format!("constant Terminal execution completed externally: {completion:?}"),
            ),
        }
    }

    fn interpreter_program(&self) -> checked_interpreter::BuildMachineProgram<'_> {
        match &self.program {
            PreparedBuildMachineBody::Typed(typed) => {
                checked_interpreter::BuildMachineProgram::Typed(typed)
            }
            PreparedBuildMachineBody::Checked(program) => {
                checked_interpreter::BuildMachineProgram::CheckedClosure {
                    execution: &program.checked,
                    product_declarations: &program.product_declarations,
                }
            }
        }
    }

    /// Bind one exact machine with closed static applications to this immutable
    /// prepared program. The lookup never falls back to a matching name. No
    /// execution mode or host grant can admit an unfinished specialization.
    pub fn entry(&self, symbol: SymbolHandle) -> Result<PreparedBuildMachineEntry, String> {
        if !self.typed().machine(symbol).is_some() {
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
    pub product_entry_compatibility: Option<&'a dyn ProductEntryCompatibility>,
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
        machine_bodies: &[],
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
