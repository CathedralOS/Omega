//! The observed end of one Psi execution. Single and batch output use the same record.
use super::TerminalOutcome;
use terminal_runtime::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalRuntimeError,
    TerminalRuntimeObservation,
};

impl From<Result<TerminalRuntimeObservation, TerminalRuntimeError>> for TerminalOutcome {
    fn from(result: Result<TerminalRuntimeObservation, TerminalRuntimeError>) -> Self {
        match result {
            Ok(observation) => Self::Completed(observation),
            Err(error) if error.unavailable_before_guest_effects() => {
                Self::Unavailable(error.to_string())
            }
            Err(error) => Self::Failed(error.to_string()),
        }
    }
}

pub enum PsiTermination {
    Returned,
    Exited(i32),
    Crashed(terminal_psi::CrashCause),
    Incomplete(String),
    Unavailable(String),
    Failed(String),
}

impl PsiTermination {
    pub(crate) fn of(outcome: &TerminalOutcome) -> Self {
        let observation = match outcome {
            TerminalOutcome::Completed(observation) => observation,
            TerminalOutcome::Unavailable(reason) => return Self::Unavailable(reason.clone()),
            TerminalOutcome::Failed(reason) => return Self::Failed(reason.clone()),
        };
        match &observation.status {
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => Self::Returned,
            TerminalExecutionStatus::ExternalCompletion(completion) => {
                Self::Exited(completion.return_code)
            }
            TerminalExecutionStatus::Crashed(crash) => Self::Crashed(crash.cause),
            TerminalExecutionStatus::Complete(result) => {
                Self::Incomplete(format!("entry returned {result:?}"))
            }
            TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                Self::Incomplete(exhaustion.to_string())
            }
            TerminalExecutionStatus::AwaitingEffect(site) => {
                Self::Incomplete(format!("awaiting host readiness at {site:?}"))
            }
            TerminalExecutionStatus::HostComputationPending(site) => {
                Self::Incomplete(format!("unfinished host computation at {site:?}"))
            }
        }
    }

    pub(crate) fn complete(&self) -> bool {
        matches!(self, Self::Returned | Self::Exited(_) | Self::Crashed(_))
    }

    pub(crate) fn columns(&self) -> (&'static str, String) {
        match self {
            Self::Returned => ("return", String::new()),
            Self::Exited(code) => ("exit", code.to_string()),
            Self::Crashed(cause) => ("crash", format!("{cause:?}")),
            Self::Incomplete(reason) => ("incomplete", reason.clone()),
            Self::Unavailable(reason) => ("unavailable", reason.clone()),
            Self::Failed(reason) => ("failed", reason.clone()),
        }
    }
}
