//! Decide each test's requirement to return, then compare the two executions when requested.
use super::TestExecution;
use crate::observations::{ProcessExitObservation, ProgramStreams, first_output_difference};
use terminal_psi::CrashCause;

#[derive(Clone)]
pub(super) enum EngineError {
    Failed(String),
    Incomplete(String),
    Unavailable(String),
}

pub(super) struct Observed<T> {
    pub termination: T,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl<T> Observed<T> {
    fn streams(&self) -> ProgramStreams<'_> {
        ProgramStreams {
            stdout: &self.stdout,
            stderr: &self.stderr,
        }
    }
}

pub(super) type Run<T> = Result<Observed<T>, EngineError>;

pub(super) enum PsiTermination {
    Returned,
    Crashed(CrashCause),
    Exited(i32),
}

pub(super) struct NativeTermination {
    pub status: ProcessExitObservation,
    pub can_exit: bool,
}

impl PsiTermination {
    fn expect_return(&self) -> Result<(), EngineError> {
        match self {
            Self::Returned => Ok(()),
            Self::Exited(code) => Err(EngineError::Failed(format!(
                "exited the process with status {code} instead of returning to the runner"
            ))),
            Self::Crashed(cause) => Err(EngineError::Failed(format!("crashed with {cause:?}"))),
        }
    }
}

impl NativeTermination {
    /// A unit return, an explicit exit and (on Windows) a trap can share a numeric status.
    /// If the program can explicitly exit, a matching number alone cannot distinguish them.
    fn matches(&self, psi: &PsiTermination) -> Option<bool> {
        let returned = self.status == ProcessExitObservation::Exited(0);
        let (matches, ambiguous) = match psi {
            PsiTermination::Returned => (Some(returned), self.can_exit),
            PsiTermination::Crashed(cause) => (
                self.status.matches_crash_presentation(*cause),
                self.can_exit && self.status.code().is_some(),
            ),
            PsiTermination::Exited(code) if self.can_exit => (
                self.status.matches_host_presentation(*code),
                // Another native i32 can have the same low byte even when Psi's status fits in it.
                cfg!(unix)
                    || returned
                    || self.status.matches_crash_presentation(CrashCause::Trap) == Some(true),
            ),
            PsiTermination::Exited(_) => return Some(false),
        };
        match matches {
            Some(true) if ambiguous => None,
            decided => decided,
        }
    }

    fn expect_return(&self) -> Result<(), EngineError> {
        match self.status {
            ProcessExitObservation::Exited(0) if self.can_exit => Err(EngineError::Incomplete(
                "exit 0 cannot distinguish a normal return from explicit process exit".into(),
            )),
            ProcessExitObservation::Exited(0) => Ok(()),
            ProcessExitObservation::Exited(code) => Err(EngineError::Failed(format!(
                "terminated with status {code} instead of returning normally"
            ))),
            other => Err(EngineError::Failed(format!(
                "terminated abnormally: {}",
                other.describe()
            ))),
        }
    }
}

fn verdict<T>(
    run: &Run<T>,
    expected: fn(&T) -> Result<(), EngineError>,
) -> Result<(), EngineError> {
    match run {
        Ok(observed) => expected(&observed.termination),
        Err(error) => Err(error.clone()),
    }
}

fn compare(psi: &Run<PsiTermination>, native: &Run<NativeTermination>) -> Result<(), String> {
    let (Ok(psi), Ok(native)) = (psi, native) else {
        return Err(
            "native/Psi comparison unavailable because both observations are required".into(),
        );
    };
    let termination = native.termination.matches(&psi.termination);
    if termination == Some(false) {
        return Err("native/Psi comparison differs in termination".into());
    }
    if let Some(difference) = first_output_difference(native.streams(), psi.streams()) {
        return Err(format!(
            "native/Psi comparison differs at {} byte {}",
            difference.stream.name(),
            difference.offset
        ));
    }
    if termination.is_none() {
        return Err("native/Psi comparison unavailable because the host cannot distinguish the exact termination".into());
    }
    Ok(())
}

pub(super) fn assess(
    psi: Run<PsiTermination>,
    native: Run<NativeTermination>,
    execution: TestExecution,
) -> Result<String, String> {
    let mut passed = Vec::new();
    let mut unavailable = Vec::new();
    let mut failures = Vec::new();
    for (engine, result) in [
        (
            "Psi interpreter",
            verdict(&psi, PsiTermination::expect_return),
        ),
        ("native", verdict(&native, NativeTermination::expect_return)),
    ] {
        match result {
            Ok(()) => passed.push(engine),
            Err(EngineError::Failed(reason)) => {
                failures.push(format!("failed on the {engine} engine: {reason}"))
            }
            Err(EngineError::Incomplete(reason)) => failures.push(format!(
                "has an incomplete result on the {engine} engine: {reason}"
            )),
            Err(EngineError::Unavailable(reason)) => {
                unavailable.push(format!("{engine}: {reason}"))
            }
        }
    }
    let mut rows = failures;
    let mut failed = !rows.is_empty() || passed.is_empty();
    if !passed.is_empty() {
        rows.push(format!("passed on {}", passed.join(", ")));
    }
    if !unavailable.is_empty() {
        if passed.is_empty() && rows.is_empty() {
            rows.push(format!(
                "has no runnable engine ({}); the test run is incomplete",
                unavailable.join("; ")
            ));
        } else {
            rows.push(format!("not run on {}", unavailable.join("; ")));
        }
    }
    if execution == TestExecution::Compare {
        match compare(&psi, &native) {
            Ok(()) => rows.push("native/Psi comparison agrees".into()),
            Err(reason) => {
                rows.push(reason);
                failed = true;
            }
        }
    }
    let summary = rows.join("; ");
    if failed { Err(summary) } else { Ok(summary) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed<T>(termination: T, stdout: &[u8], stderr: &[u8]) -> Run<T> {
        Ok(Observed {
            termination,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
        })
    }

    #[test]
    fn equal_returns_cannot_hide_binary_stream_differences() {
        let psi = observed(PsiTermination::Returned, &[4, 0, 255], &[9]);
        let native = observed(
            NativeTermination {
                status: ProcessExitObservation::Exited(0),
                can_exit: false,
            },
            &[4, 0, 254],
            &[8],
        );
        let error = assess(psi, native, TestExecution::Compare).unwrap_err();
        assert!(
            error.contains("passed on Psi interpreter, native"),
            "{error}"
        );
        assert!(error.contains("differs at stdout byte 2"), "{error}");
    }

    #[test]
    fn a_different_visible_exit_status_proves_disagreement() {
        let native = NativeTermination {
            status: ProcessExitObservation::Exited(7),
            can_exit: true,
        };
        assert_eq!(native.matches(&PsiTermination::Exited(23)), Some(false));
        assert_eq!(native.matches(&PsiTermination::Returned), Some(false));
    }

    #[test]
    fn an_ambiguous_exit_cannot_hide_a_known_stream_difference() {
        let native = observed(
            NativeTermination {
                status: ProcessExitObservation::Exited(0),
                can_exit: true,
            },
            &[],
            b"1",
        );
        let psi = observed(PsiTermination::Returned, &[], b"2");
        let error = assess(psi, native, TestExecution::Compare).unwrap_err();
        assert!(error.contains("differs at stderr byte 0"), "{error}");
    }

    #[test]
    fn a_single_pass_is_incomplete_only_when_both_engines_were_requested() {
        let run = |mode| {
            assess(
                observed(PsiTermination::Returned, &[], &[]),
                Err(EngineError::Unavailable("cross target".into())),
                mode,
            )
        };
        assert!(
            run(TestExecution::Available)
                .unwrap()
                .contains("cross target")
        );
        let incomplete = run(TestExecution::Compare).unwrap_err();
        assert!(
            incomplete.contains("comparison unavailable"),
            "{incomplete}"
        );
        assert!(incomplete.contains("cross target"), "{incomplete}");
    }
}
