//! Collect a registered native test's output and reap its process container.
use super::result::EngineError;
use crate::NativeEntryLimits;
use bounded_process::{
    BoundedCaptureBudget, BoundedProcessInput, BoundedProcessOutput, BoundedProcessPrepared,
    BoundedProcessRunError, run_bounded_process,
};
use std::process::Command;

pub(super) fn capture(
    command: Command,
    limits: NativeEntryLimits,
    private_root: Option<&std::path::Path>,
) -> Result<BoundedProcessOutput, EngineError> {
    let mut prepared =
        BoundedProcessPrepared::new(command, limits.process, "registered native test")
            .map_err(|error| EngineError::Failed(format!("cannot prepare the test: {error}")))?;
    let stdin = if let Some(scratch) = private_root {
        prepared
            .use_private_filesystem(scratch, Default::default())
            .map_err(|error| {
                EngineError::Unavailable(format!(
                    "cannot prepare the private test filesystem: {error}"
                ))
            })?;
        BoundedProcessInput::Bytes(Vec::new())
    } else {
        BoundedProcessInput::Null
    };
    run_bounded_process(
        prepared,
        stdin,
        limits.capture,
        BoundedCaptureBudget::new(limits.combined_output_bytes),
    )
    .map_err(|error| match error {
        BoundedProcessRunError::Spawn(reason) => {
            EngineError::Unavailable(format!("cannot start the test: {reason}"))
        }
        BoundedProcessRunError::TimedOut { timeout } => EngineError::Incomplete(format!(
            "execution reached its deadline under the {timeout:?} run and cleanup allowance"
        )),
        BoundedProcessRunError::OutputOverflow { stream, limit } => {
            EngineError::Incomplete(format!(
                "{} exceeded its {limit}-byte capture allowance",
                stream.name()
            ))
        }
        BoundedProcessRunError::AggregateOutputOverflow { ceiling, .. } => EngineError::Incomplete(
            format!("stdout and stderr exceeded their combined {ceiling}-byte capture allowance"),
        ),
        BoundedProcessRunError::InvalidLimits => {
            EngineError::Failed("the test sponsor supplied invalid capture limits".into())
        }
        other => {
            EngineError::Incomplete(format!("cannot collect the running test's result: {other}"))
        }
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::time::Duration;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn each_execution_has_its_own_bounded_streams() {
        let mut limits = NativeEntryLimits::INSTALLED;
        limits.capture.stdout_bytes = 3;
        limits.capture.stderr_bytes = 3;
        limits.combined_output_bytes = 6;

        let run = || capture(shell("printf out; printf err >&2"), limits, None).unwrap();
        for output in [run(), run()] {
            assert!(output.status.success());
            assert_eq!(output.stdout, b"out");
            assert_eq!(output.stderr, b"err");
        }
        let overflow = capture(shell("printf more >&2"), limits, None).unwrap_err();
        assert!(
            matches!(&overflow, EngineError::Incomplete(reason) if reason.contains("stderr exceeded its 3-byte capture allowance")),
            "{overflow:?}"
        );
        limits.combined_output_bytes = 5;
        let overflow = capture(shell("printf out; printf err >&2"), limits, None).unwrap_err();
        assert!(
            matches!(&overflow, EngineError::Incomplete(reason) if reason.contains("combined 5-byte capture allowance")),
            "{overflow:?}"
        );
    }

    #[test]
    fn only_a_test_that_started_can_exhaust_its_deadline() {
        let mut limits = NativeEntryLimits::INSTALLED;
        limits.capture.timeout = Duration::from_secs(1);
        let timeout = capture(shell("exec sleep 20"), limits, None).unwrap_err();
        assert!(
            matches!(&timeout, EngineError::Incomplete(reason) if reason.contains("deadline")),
            "{timeout:?}"
        );
        let absent = capture(Command::new("/nonexistent/omega-test"), limits, None).unwrap_err();
        assert!(matches!(absent, EngineError::Unavailable(_)), "{absent:?}");
    }
}
