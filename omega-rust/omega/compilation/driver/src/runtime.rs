//! Execute the produced Terminal program under an explicit isolated host root.
//!
//! The native producer retains its admitted entry and provider provisions.
//! Reference execution consumes those exact records without rebuilding source
//! or rerunning Build. They identify what to establish; this owner creates the
//! actual zero backing, pure buffered host bindings and containing-domain exit
//! delegation. Guest storage never escapes, no placed host loans are admitted,
//! and host output is copied into an observation rather than performed on a
//! surviving process. Those are the domain-death premises of this runtime, not
//! permissions inferred from a provider name or from a reach declaration.

mod entry;
mod host_imports;

use compilation_products::TerminalRuntimeProvisions;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalBindingRejection, TerminalExecutionStatus,
    TerminalInstanceError, TerminalInterpretError, TerminalProcessExitAdmission, TerminalProgram,
};

pub struct TerminalRuntimeRequest<'program> {
    pub artifact: &'program terminal_codec::CanonicalTerminalArtifact,
    pub provisions: Option<&'program TerminalRuntimeProvisions>,
    pub fuel_allowance: u64,
}

/// The exact semantic outcome and preceding buffered output. An external exit
/// retains its full i32 argument; physical status presentation belongs to the
/// consumer's host contract, not this reference execution.
pub struct TerminalRuntimeObservation {
    pub status: TerminalExecutionStatus,
    pub stdout: Vec<u8>,
}

#[derive(Debug)]
pub enum TerminalRuntimeError {
    MissingProvision {
        capability: &'static str,
        reason: String,
    },
    InvalidProvision(&'static str),
    Artifact(TerminalArtifactInterpretError),
    Requirements(TerminalInterpretError),
    Binding(TerminalBindingRejection),
    Execution(TerminalInstanceError),
}

impl TerminalRuntimeError {
    pub(super) fn missing(capability: &'static str, reason: impl Into<String>) -> Self {
        Self::MissingProvision {
            capability,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for TerminalRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProvision { capability, reason } => {
                write!(formatter, "{capability}: {reason}")
            }
            Self::InvalidProvision(reason) => formatter.write_str(reason),
            Self::Artifact(error) => std::fmt::Display::fmt(error, formatter),
            Self::Requirements(error) => std::fmt::Display::fmt(error, formatter),
            Self::Binding(error) => write!(formatter, "runtime binding rejected: {error:?}"),
            Self::Execution(error) => write!(formatter, "runtime execution failed: {error:?}"),
        }
    }
}

impl std::error::Error for TerminalRuntimeError {}

/// Establish this driver's actual host root before invoking any guest effect.
/// Absence of retained provisions permits only a genuinely input/import-free
/// Terminal program; it never enables synthetic context or host bindings.
pub fn execute(
    request: TerminalRuntimeRequest<'_>,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let program = TerminalProgram::load(request.artifact.semantic_bytes())
        .map_err(TerminalRuntimeError::Artifact)?;
    if let Some(provisions) = request.provisions
        && (provisions.identity() != program.terminal_psi()
            || provisions.entry()
                != request
                    .artifact
                    .semantic_module()
                    .map_err(|_| {
                        TerminalRuntimeError::InvalidProvision("Terminal module unavailable")
                    })?
                    .entry)
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "runtime provisions name another Terminal program or entry",
        ));
    }
    let installation = request
        .provisions
        .and_then(TerminalRuntimeProvisions::installation);
    let requirements = program
        .execution_requirements(installation)
        .map_err(TerminalRuntimeError::Requirements)?;
    if !requirements.missing_providers.is_empty() {
        return Err(TerminalRuntimeError::missing(
            "runtime-provider-installation",
            format!(
                "uninstalled exact provider slots {:?}",
                requirements.missing_providers
            ),
        ));
    }
    let arrival = entry::establish(&program, request.provisions)?;
    let inputs = arrival.inputs();
    let mut instance = match installation {
        Some(installation) => program.instantiate_installed(&[], inputs, installation),
        None => program.instantiate(&[], inputs),
    }
    .map_err(TerminalRuntimeError::Artifact)?;
    let output_slots = host_imports::bind(
        &program,
        &mut instance,
        request.provisions,
        &requirements.imports,
    )?;
    // Each Binding occurrence is established for this containing domain by
    // this host, never by merely selecting the native exit implementation.
    for demand in &requirements.external_completions {
        if !arrival.established_slots().contains(&demand.boundary) {
            return Err(TerminalRuntimeError::missing(
                "process-exit-domain-binding",
                format!(
                    "exit slot {:?} has no exact established root Binding occurrence",
                    demand.boundary
                ),
            ));
        }
        let admission = TerminalProcessExitAdmission::admit_isolated_simulated_domain(
            &instance,
            demand.boundary,
        )
        .map_err(TerminalRuntimeError::Binding)?;
        instance
            .bind_external_completion(demand.boundary, admission)
            .map_err(TerminalRuntimeError::Binding)?;
    }
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(request.fuel_allowance);
    let status = instance
        .invoke(&mut fuel)
        .map_err(TerminalRuntimeError::Execution)?;
    let stdout = host_imports::collect_stdout(instance.effects(), &output_slots)?;
    Ok(TerminalRuntimeObservation { status, stdout })
}
