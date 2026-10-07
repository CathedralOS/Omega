//! The corpus runtime leg drives the same admitted program/instance as embedding.
//!
//! Compilation/package review stay in the corpus runner. This adapter consumes
//! the genuine artifact and producer admission profile; it never replays Build,
//! supplies dummy context, falls back to checked trees, or turns execution errors
//! into missing coverage. Driver provisions not implemented yet are identified
//! from the exact admitted input/import contract before any operation or effect.
//! External process completion still needs a terminal-invocation representation:
//! a normal Unit return or a host callback returning Unit cannot impersonate it.

use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalProgram, TerminalStructuralInputs,
    TerminalUnsupportedCapability,
};

#[derive(Default)]
pub(super) struct Observation {
    pub facts: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl Observation {
    pub fn failure(error: impl std::fmt::Display) -> Self {
        Self {
            facts: vec!["interpreter:declined".to_owned()],
            diagnostics: vec![format!("Terminal interpreter: {error}")],
        }
    }

    fn missing(capability: &str, task: &str, reason: impl std::fmt::Display) -> Self {
        Self {
            facts: Vec::new(),
            diagnostics: vec![format!(
                "unimplemented: capability={capability} task={task} route=interpreter: {reason}"
            )],
        }
    }
}

pub(super) fn interpret(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    expected_stdout: Option<&str>,
    fuel_allowance: u64,
) -> Observation {
    let program = match TerminalProgram::load(artifact.semantic_bytes()) {
        Ok(program) => program,
        Err(error) => return Observation::failure(error),
    };
    let requirements = match program.execution_requirements(None) {
        Ok(requirements) => requirements,
        Err(error) => {
            return match error {
                TerminalInterpretError::UnsupportedCapability(
                    TerminalUnsupportedCapability::ExternalCompletionInvocation { boundary, kind },
                ) => Observation::missing(
                    "external-completion-invocation",
                    "TERMINAL-RUNTIME-DRIVERS",
                    format!(
                        "boundary {boundary:?} demands {kind:?}; its closed terminal invocation is missing"
                    ),
                ),
                TerminalInterpretError::UnsupportedCapability(
                    TerminalUnsupportedCapability::SelectedReferenceResultBacking {
                        machine,
                        result_path,
                    },
                ) => Observation::missing(
                    "selected-reference-result-backing",
                    "TERMINAL-CONST-STRUCTURES",
                    format!(
                        "machine {machine:?} result leaf {result_path:?} needs alternative backing transport"
                    ),
                ),
                other => Observation::failure(other),
            };
        }
    };
    if !requirements.missing_providers.is_empty() {
        return Observation::missing(
            "runtime-provider-installation",
            "TERMINAL-RUNTIME-DRIVERS",
            format!(
                "exact reachable provider requirements {:?} need the runtime installation adapter",
                requirements.missing_providers
            ),
        );
    }
    let entry = program.entry();
    if !entry.scalar_parameters.is_empty()
        || !entry.structural_parameters.is_empty()
        || !entry.placed_view_inputs.is_empty()
    {
        return Observation::missing(
            "program-entry-arrival-values",
            "TERMINAL-RUNTIME-DRIVERS",
            format!(
                "the admitted entry demands {} scalar values, {} structural values and {} placed-view establishments; the corpus runtime provision adapter is missing",
                entry.scalar_parameters.len(),
                entry.structural_parameters.len(),
                entry.placed_view_inputs.len()
            ),
        );
    }
    if let Some(slot) = requirements.imports.first() {
        let Some(import) = program.import(*slot) else {
            return Observation::failure("reachable import has no admitted declaration");
        };
        return Observation::missing(
            "runtime-host-import-bindings",
            "TERMINAL-RUNTIME-DRIVERS",
            format!(
                "exact import {:?} ({}) demands {:?}; its exact runtime host binding adapter is missing",
                import.id, import.identity, import.completion,
            ),
        );
    }
    let mut instance = match program.instantiate(&[], TerminalStructuralInputs::default()) {
        Ok(instance) => instance,
        // This branch is an execution/input failure, never a migration marker.
        Err(TerminalArtifactInterpretError::Execution(error)) => {
            return Observation::failure(error);
        }
        Err(other) => return Observation::failure(other),
    };
    let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(fuel_allowance);
    let status = match instance.invoke(&mut fuel) {
        Ok(status) => status,
        Err(error) => return Observation::failure(format!("{error:?}")),
    };
    let mut observation = match status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => Observation {
            facts: vec!["interpreter-exit:0".to_owned()],
            diagnostics: Vec::new(),
        },
        TerminalExecutionStatus::Crashed(crash)
            if crash.cause == terminal_psi::CrashCause::Trap =>
        {
            Observation {
                facts: vec!["interpreter:trapped".to_owned()],
                diagnostics: Vec::new(),
            }
        }
        TerminalExecutionStatus::SponsorExhausted(_) => {
            return Observation {
                facts: vec!["interpreter:exhausted".to_owned()],
                diagnostics: Vec::new(),
            };
        }
        other => {
            return Observation::failure(format!(
                "entry outcome is not process completion: {other:?}"
            ));
        }
    };
    // There are no demanded host imports on this completed route. Runtime
    // port operations are rejected by the shared instance dispatch rather than
    // silently erased; therefore the observed stdout here is exactly empty.
    if let Some(expected) = expected_stdout {
        observation
            .facts
            .push(if expected.replace("\r\n", "\n").is_empty() {
                "interpreter-stdout:match".to_owned()
            } else {
                "interpreter-stdout:differs".to_owned()
            });
    }
    observation
}
