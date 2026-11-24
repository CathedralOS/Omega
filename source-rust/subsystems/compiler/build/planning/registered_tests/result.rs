//! Decide each test's requirement to return, then compare the two executions when requested.
use super::TestExecution;
use crate::observations::{
    ProcessExitObservation, ProgramStream, ProgramStreams, first_byte_difference,
};
use source_compiler::registered_tests::{TestExpectations, TestFileExpectation};
use terminal_psi::CrashCause;

#[derive(Clone, Debug)]
pub(super) enum EngineError {
    Failed(String),
    Incomplete(String),
    Unavailable(String),
}

pub(super) struct Observed<T> {
    pub termination: T,
    pub files: terminal_runtime::ObservedFiles,
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
    expect_return: fn(&T) -> Result<(), EngineError>,
    expected: &TestExpectations,
) -> Result<(), EngineError> {
    let observed = run.as_ref().map_err(Clone::clone)?;
    let termination = expect_return(&observed.termination);
    let mut mismatches = Vec::new();
    for (stream, wanted, actual) in [
        (
            ProgramStream::Stdout,
            expected.stdout.as_deref(),
            &observed.stdout,
        ),
        (
            ProgramStream::Stderr,
            expected.stderr.as_deref(),
            &observed.stderr,
        ),
    ] {
        if let Some(bytes) = wanted
            && let Some(offset) = first_byte_difference(bytes, actual)
        {
            let show = |bytes: &[u8]| match bytes.get(offset) {
                Some(byte) => format!("{byte:#04x}"),
                None => "end of stream".into(),
            };
            mismatches.push(format!(
                "{} byte {offset}: expected {}, observed {}",
                stream.name(),
                show(bytes),
                show(actual)
            ));
        }
    }
    for (path, wanted) in &expected.files {
        let name = String::from_utf8_lossy(path);
        let Some(actual) = observed.files.get(path) else {
            mismatches.push(format!("file `{name}` has no observation"));
            continue;
        };
        let difference = match wanted {
            TestFileExpectation::Observe => None,
            TestFileExpectation::Missing => file_difference(None, actual.as_deref()),
            TestFileExpectation::Bytes(bytes) => file_difference(Some(bytes), actual.as_deref()),
        };
        if let Some(difference) = difference {
            mismatches.push(format!("file `{name}` {difference}"));
        }
    }
    if mismatches.is_empty() {
        return termination;
    }
    if let Err(
        EngineError::Failed(reason)
        | EngineError::Incomplete(reason)
        | EngineError::Unavailable(reason),
    ) = termination
    {
        mismatches.push(reason);
    }
    Err(EngineError::Failed(mismatches.join("; ")))
}

/// The first operand is the independent oracle, or the Psi result during comparison.
fn file_difference(expected: Option<&[u8]>, observed: Option<&[u8]>) -> Option<String> {
    match (expected, observed) {
        (None, None) => None,
        (None, Some(bytes)) => Some(format!("expected missing, observed {} bytes", bytes.len())),
        (Some(bytes), None) => Some(format!("expected {} bytes, observed missing", bytes.len())),
        (Some(expected), Some(actual)) => first_byte_difference(expected, actual).map(|offset| {
            let show = |bytes: &[u8]| {
                bytes
                    .get(offset)
                    .map_or_else(|| "end of file".to_owned(), |byte| format!("{byte:#04x}"))
            };
            format!(
                "differs at byte {offset}: expected {}, observed {}",
                show(expected),
                show(actual)
            )
        }),
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
    if let Some((stream, offset)) = native.streams().first_difference(psi.streams()) {
        return Err(format!(
            "native/Psi comparison differs at {} byte {}",
            stream.name(),
            offset
        ));
    }
    if psi.files.keys().ne(native.files.keys()) {
        return Err(
            "native/Psi comparison unavailable because the observed file names differ".into(),
        );
    }
    for (path, psi_file) in &psi.files {
        if let Some(difference) =
            file_difference(psi_file.as_deref(), native.files[path].as_deref())
        {
            return Err(format!(
                "native/Psi comparison for file `{}` {difference} (Psi, native)",
                String::from_utf8_lossy(path)
            ));
        }
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
    expected: &TestExpectations,
) -> Result<String, String> {
    let mut passed = Vec::new();
    let mut unavailable = Vec::new();
    let mut failures = Vec::new();
    for (engine, result) in [
        (
            "Psi interpreter",
            verdict(&psi, PsiTermination::expect_return, expected),
        ),
        (
            "native",
            verdict(&native, NativeTermination::expect_return, expected),
        ),
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
            files: Default::default(),
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
        let error = assess(psi, native, TestExecution::Compare, &Default::default()).unwrap_err();
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
        let error = assess(psi, native, TestExecution::Compare, &Default::default()).unwrap_err();
        assert!(error.contains("differs at stderr byte 0"), "{error}");
    }

    #[test]
    fn a_single_pass_is_incomplete_only_when_both_engines_were_requested() {
        let run = |mode| {
            assess(
                observed(PsiTermination::Returned, &[], &[]),
                Err(EngineError::Unavailable("cross target".into())),
                mode,
                &Default::default(),
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
