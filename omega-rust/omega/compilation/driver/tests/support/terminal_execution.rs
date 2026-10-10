//! The corpus runtime leg uses the shipped Terminal runtime owner. It supplies
//! the native report's exact retained provisions, never a second compilation,
//! dummy receiver, expected-output callback or Unit-returning exit mock.

use compiler::runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeRequest,
};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

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
    provisions: Option<&compilation_products::TerminalRuntimeProvisions>,
    target: Option<&str>,
    fixture_dir: &std::path::Path,
    fuel_allowance: u64,
    standard_input: Box<dyn std::io::Read + Send>,
) -> Observation {
    let clock = match compiler::runtime::TerminalClockSnapshot::capture_host_epoch() {
        Ok(clock) => clock,
        Err(error) => return Observation::failure(error),
    };
    let result = match compiler::runtime::execute(TerminalRuntimeRequest {
        artifact,
        target,
        provisions,
        fuel_allowance,
        domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: Some(standard_input),
        clock: Some(clock),
    }) {
        Ok(result) => result,
        Err(TerminalRuntimeError::MissingProvision { capability, reason }) => {
            return Observation::missing(capability, "TERMINAL-RUNTIME-DRIVERS", reason);
        }
        Err(error) => return Observation::failure(error),
    };
    let mut observation = match result.status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => Observation {
            facts: vec!["interpreter-exit:0".to_owned()],
            diagnostics: Vec::new(),
        },
        TerminalExecutionStatus::ExternalCompletion(completion) => Observation {
            facts: vec![format!("interpreter-exit:{}", completion.return_code)],
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
    observation.facts.extend(super::output_oracles::observe(
        fixture_dir,
        "interpreter-",
        Some(&result.stdout),
        Some(&result.stderr),
    ));
    observation
}
