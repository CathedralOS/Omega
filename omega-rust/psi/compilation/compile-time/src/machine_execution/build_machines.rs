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
    program: Arc<PreparedBuildMachineBody>,
    identity: Arc<PreparedBuildMachineProgramIdentity>,
}

#[derive(Clone)]
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

impl CheckedBuildProgram {
    pub(crate) fn typed(&self) -> &TypedTrees {
        &self.checked.typed
    }

    pub fn checked(&self) -> &checked_trees::CheckedTrees {
        &self.checked
    }

    pub(crate) fn interpreter_program(&self) -> checked_interpreter::BuildMachineProgram<'_> {
        checked_interpreter::BuildMachineProgram::CheckedClosure {
            execution: &self.checked,
            product_declarations: &self.product_declarations,
        }
    }
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
        Self::from_checked_shared(Arc::new(program))
    }

    /// Retain the actual shared checked output without copying its arenas.
    /// This ordinary content entrance grants no selected Build role.
    pub fn from_checked_shared(program: Arc<checked_trees::CheckedTrees>) -> Self {
        let product_declarations = program
            .typed
            .product_declaration_frontier
            .clone()
            .unwrap_or_else(|| Arc::new(program.typed.as_ref().clone()));
        Self {
            program: Arc::new(PreparedBuildMachineBody::Checked(CheckedBuildProgram {
                checked: program,
                product_declarations,
            })),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        }
    }

    /// Execution settlement may replace only its selected comparison facts.
    /// The checked declarations stay owned; the facade retains its projection.
    pub fn with_selected_comparison_executions(
        self,
        executions: Vec<checked_trees::CheckedSelectedFloatComparisonExecution>,
    ) -> Self {
        let mut body = Arc::try_unwrap(self.program).unwrap_or_else(|shared| (*shared).clone());
        if let PreparedBuildMachineBody::Checked(program) = &mut body {
            Arc::make_mut(&mut program.checked)
                .facts
                .operators
                .replace_selected_float_comparisons(executions);
        }
        Self {
            program: Arc::new(body),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        }
    }

    /// Fix the complete invocation before effects. Ordinary proof, constant and
    /// helper calls acquire no selected Build role or source-root record.
    pub fn activation(
        &self,
        entry: &PreparedBuildMachineEntry,
        arguments: Vec<BuildTimeValue>,
        mode: BuildMachineExecutionMode,
    ) -> Result<PreparedBuildMachineActivation, BuildMachineEvaluationError> {
        self.validate_entry(entry)?;
        let description = None;
        Ok(PreparedBuildMachineActivation {
            program: Arc::clone(&self.program),
            entry: entry.clone(),
            arguments,
            mode,
            description,
            identity: BuildMachineActivationIdentity(Arc::new(BuildMachineActivationOwner {
                _private: 0,
            })),
        })
    }

    /// Capture this actual source entry before effects. The caller still owes
    /// the selected Build role; a raw evaluation record grants none.
    pub fn source_root_activation(
        &self,
        entry: &PreparedBuildMachineEntry,
        arguments: Vec<BuildTimeValue>,
        mode: BuildMachineExecutionMode,
    ) -> Result<PreparedBuildMachineActivation, BuildMachineEvaluationError> {
        let mut activation = self.activation(entry, arguments, mode)?;
        activation.description = Some(PreparedBuildRootDescription::capture(
            self,
            self.validate_entry(entry)?,
        )?);
        Ok(activation)
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
            program: Arc::new(PreparedBuildMachineBody::Typed(Box::new(typed))),
            identity: Arc::new(PreparedBuildMachineProgramIdentity { _private: 0 }),
        })
    }

    pub fn typed(&self) -> &TypedTrees {
        match self.program.as_ref() {
            PreparedBuildMachineBody::Typed(typed) => typed,
            PreparedBuildMachineBody::Checked(program) => &program.checked.typed,
        }
    }

    /// Retain the complete immutable checked program, including CheckFacts.
    /// This is content only, never the preparation's executable entry token.
    pub fn checked_program(&self) -> Option<CheckedBuildProgram> {
        match self.program.as_ref() {
            PreparedBuildMachineBody::Checked(program) => Some(program.clone()),
            PreparedBuildMachineBody::Typed(_) => None,
        }
    }

    /// Return one admitted zero-argument constant through this checked
    /// preparation's exact entry identity. The caller still owns ConstEvaluable
    /// and package selection admission; this entrance retains every CheckFact
    /// and selected operation while the existing evaluator remains in use.
    pub(crate) fn evaluate_checked_constant(
        &self,
        entry: &PreparedBuildMachineEntry,
        operators: &[crate::SelectedBuildTimeBinaryOperator],
    ) -> Result<crate::MeasuredEvaluation<BuildTimeValue>, String> {
        if !matches!(self.program.as_ref(), PreparedBuildMachineBody::Checked(_)) {
            return Err("constant invocation requires a checked preparation".into());
        }
        let symbol = self
            .validate_entry(entry)
            .map_err(|error| error.to_string())?;
        super::selected_operators::validate_selected_operators(self.typed(), operators)?;
        checked_interpreter::evaluate_prepared_build_time_machine(
            self.interpreter_program(),
            checked_interpreter::BuildMachineEvaluationRequest {
                operators,
                ..checked_interpreter::BuildMachineEvaluationRequest::symbol(symbol, Vec::new())
            },
        )
        .map(checked_interpreter::BuildTimeOperationEvaluation::into_measured)
    }

    fn interpreter_program(&self) -> checked_interpreter::BuildMachineProgram<'_> {
        match self.program.as_ref() {
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

/// Identity of one privately fixed invocation, separate from entry and cache
/// identity. Two admissions of the same source/profile still name different
/// activations until they genuinely share this exact private record.
#[derive(Clone)]
pub struct BuildMachineActivationIdentity(Arc<BuildMachineActivationOwner>);
struct BuildMachineActivationOwner {
    _private: u8,
}
impl std::fmt::Debug for BuildMachineActivationIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BuildMachineActivationIdentity(..)")
    }
}
impl PartialEq for BuildMachineActivationIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for BuildMachineActivationIdentity {}

/// Exact prepared entry, arguments and execution mode owned before effects.
/// No consumer can attach this identity to a replacement argument/mode tuple.
pub struct PreparedBuildMachineActivation {
    program: Arc<PreparedBuildMachineBody>,
    entry: PreparedBuildMachineEntry,
    arguments: Vec<BuildTimeValue>,
    mode: BuildMachineExecutionMode,
    description: Option<PreparedBuildRootDescription>,
    identity: BuildMachineActivationIdentity,
}
impl PreparedBuildMachineActivation {
    pub fn identity(&self) -> BuildMachineActivationIdentity {
        self.identity.clone()
    }
    pub fn arguments(&self) -> &[BuildTimeValue] {
        &self.arguments
    }
    pub fn mode(&self) -> &BuildMachineExecutionMode {
        &self.mode
    }
}

#[derive(Clone)]
struct PreparedBuildRootDescription {
    owner: Arc<PreparedBuildMachineBody>,
    symbol: SymbolHandle,
    callable_identity: String,
    source_span: source::SourceSpan,
}
impl PreparedBuildRootDescription {
    fn capture(
        program: &PreparedBuildMachineProgram,
        symbol: SymbolHandle,
    ) -> Result<Self, BuildMachineEvaluationError> {
        let typed = program.typed();
        let machine = typed.machine(symbol).ok_or_else(|| {
            BuildMachineEvaluationError::Entry(
                "executed Build entry lost its original declaration".to_owned(),
            )
        })?;
        let callable_identity = typed
            .machine_callable_identity_text(machine)
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| {
                BuildMachineEvaluationError::Entry(
                    "executed Build entry has no original canonical identity".to_owned(),
                )
            })?;
        let source_span = typed
            .symbols
            .symbol_provenance_source_span(symbol)
            .ok_or_else(|| {
                BuildMachineEvaluationError::Entry(
                    "executed Build entry has no original source occurrence".to_owned(),
                )
            })?;
        let file = typed.symbols.source_file(source_span).ok_or_else(|| {
            BuildMachineEvaluationError::Entry(
                "executed Build entry lost its original source owner".to_owned(),
            )
        })?;
        if source_span.span.start >= source_span.span.end
            || source_span.span.end > file.source.len()
            || !file.source.is_char_boundary(source_span.span.start)
            || !file.source.is_char_boundary(source_span.span.end)
        {
            return Err(BuildMachineEvaluationError::Entry(
                "executed Build entry has no complete original source span".to_owned(),
            ));
        }
        Ok(Self {
            owner: Arc::clone(&program.program),
            symbol,
            callable_identity,
            source_span,
        })
    }
}

/// Published only by successful evaluation of the privately fixed activation.
/// Description validation happened before the evaluator could perform effects.
#[derive(Clone)]
pub struct ExecutedBuildMachineRoot {
    description: PreparedBuildRootDescription,
    activation: BuildMachineActivationIdentity,
}
impl ExecutedBuildMachineRoot {
    pub const fn symbol(&self) -> SymbolHandle {
        self.description.symbol
    }
    pub fn callable_identity(&self) -> &str {
        &self.description.callable_identity
    }
    pub const fn source_span(&self) -> source::SourceSpan {
        self.description.source_span
    }
    pub fn activation_identity(&self) -> &BuildMachineActivationIdentity {
        &self.activation
    }
    pub fn owner(&self) -> &TypedTrees {
        match self.description.owner.as_ref() {
            PreparedBuildMachineBody::Typed(owner) => owner,
            PreparedBuildMachineBody::Checked(owner) => owner.typed(),
        }
    }
}
impl std::fmt::Debug for ExecutedBuildMachineRoot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutedBuildMachineRoot")
            .field("symbol", &self.symbol())
            .field("callable_identity", &self.callable_identity())
            .field("source_span", &self.source_span())
            .field("activation", &self.activation)
            .finish_non_exhaustive()
    }
}
impl PartialEq for ExecutedBuildMachineRoot {
    fn eq(&self, other: &Self) -> bool {
        self.activation == other.activation
            && Arc::ptr_eq(&self.description.owner, &other.description.owner)
            && self.symbol() == other.symbol()
            && self.callable_identity() == other.callable_identity()
            && self.source_span() == other.source_span()
    }
}
impl Eq for ExecutedBuildMachineRoot {}

pub struct ExecutedBuildMachineEvaluation<T> {
    measured: MeasuredBuildMachineEvaluation<T>,
    root: Option<ExecutedBuildMachineRoot>,
}
impl<T> ExecutedBuildMachineEvaluation<T> {
    pub fn into_parts(
        self,
    ) -> (
        MeasuredBuildMachineEvaluation<T>,
        Option<ExecutedBuildMachineRoot>,
    ) {
        (self.measured, self.root)
    }
}

/// One measured build-machine invocation: the exact prepared entry, its arguments, the
/// pure or granted execution mode, and the sponsor that pays for its custody.
pub struct BuildMachineInvocation<'a> {
    pub activation: &'a PreparedBuildMachineActivation,
    pub sponsor: Option<&'a BuildEvaluationSponsor>,
    pub product_entry_compatibility: Option<&'a dyn checked_interpreter::ProductEntryCompatibility>,
}

/// Evaluate one build machine and measure its evaluation. All build-machine
/// evaluations enter here: the mode selects the pure or filesystem-granted
/// interpreter and the sponsor, when present, is charged for result custody.
pub fn evaluate_build_machine_measured(
    program: &PreparedBuildMachineProgram,
    invocation: BuildMachineInvocation<'_>,
) -> Result<ExecutedBuildMachineEvaluation<Vec<BuildTimeValue>>, BuildMachineEvaluationError> {
    if !Arc::ptr_eq(&program.program, &invocation.activation.program) {
        return Err(BuildMachineEvaluationError::Entry(
            "fixed activation belongs to a different immutable prepared execution body".to_owned(),
        ));
    }
    let symbol = program.validate_entry(&invocation.activation.entry)?;
    let request = checked_interpreter::BuildMachineEvaluationRequest {
        entry: checked_interpreter::BuildMachineEntry::Symbol(symbol),
        arguments: invocation.activation.arguments.clone(),
        operators: &[],
        sponsor: invocation.sponsor,
        product_entry_compatibility: invocation.product_entry_compatibility,
    };
    let measured = match invocation.activation.mode.clone() {
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
    }?;
    let root = invocation
        .activation
        .description
        .as_ref()
        .map(|description| ExecutedBuildMachineRoot {
            description: description.clone(),
            activation: invocation.activation.identity.clone(),
        });
    Ok(ExecutedBuildMachineEvaluation { measured, root })
}
