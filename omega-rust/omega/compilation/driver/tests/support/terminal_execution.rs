//! The corpus runtime leg uses the shipped Terminal runtime owner. It supplies
//! the native report's exact retained provisions, never a second compilation,
//! dummy receiver, expected-output callback or Unit-returning exit mock.

use compiler::runtime::{TerminalRuntimeError, TerminalRuntimeRequest};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
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
    provisions: Option<&compilation_products::TerminalRuntimeProvisions>,
    expected_stdout: Option<&str>,
    fuel_allowance: u64,
) -> Observation {
    let result = match compiler::runtime::execute(TerminalRuntimeRequest {
        artifact,
        provisions,
        fuel_allowance,
    }) {
        Ok(result) => result,
        Err(TerminalRuntimeError::MissingProvision { capability, reason }) => {
            return Observation::missing(capability, "TERMINAL-RUNTIME-DRIVERS", reason);
        }
        Err(TerminalRuntimeError::Requirements(TerminalInterpretError::UnsupportedCapability(
            TerminalUnsupportedCapability::SelectedReferenceResultBacking {
                machine,
                result_path,
            },
        ))) => {
            return Observation::missing(
                "selected-reference-result-backing",
                "TERMINAL-CONST-STRUCTURES",
                format!(
                    "machine {machine:?} result leaf {result_path:?} needs alternative backing transport"
                ),
            );
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
    if let Some(expected) = expected_stdout {
        let matches = std::str::from_utf8(&result.stdout)
            .is_ok_and(|stdout| stdout.replace("\r\n", "\n") == expected.replace("\r\n", "\n"));
        observation.facts.push(if matches {
            "interpreter-stdout:match".to_owned()
        } else {
            "interpreter-stdout:differs".to_owned()
        });
    }
    observation
}
