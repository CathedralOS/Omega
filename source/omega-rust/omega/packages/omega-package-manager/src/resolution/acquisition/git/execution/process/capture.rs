//! Bounded stream capture, deadlines, and process-group cleanup.

use crate::resolution::acquisition::SourceResolveError;
use crate::resolution::acquisition::git::execution::executable::{
    CapturedOutputLimitExceeded, GitCapturedOutputBudget,
};
use crate::resolution::acquisition::limits::{GIT_COMMAND_CLEANUP_TIMEOUT, PROCESS_POLL_INTERVAL};
use omega_resolver_execution::{
    ResolverExecutionChild, ResolverExecutionCompletionObservation, ResolverPreparedExecution,
};
use std::io::Read;
use std::process::{ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(in crate::resolution::acquisition) struct BoundedCommandOutput {
    pub(in crate::resolution::acquisition) status: ExitStatus,
    pub(in crate::resolution::acquisition) stdout: Vec<u8>,
    pub(in crate::resolution::acquisition) stderr: Vec<u8>,
    pub(in crate::resolution::acquisition) completion: ResolverExecutionCompletionObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturedStream {
    Stdout,
    Stderr,
}

impl CapturedStream {
    fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

pub(in crate::resolution::acquisition) enum StreamCaptureResult {
    Complete(Vec<u8>),
    Overflow,
    ResolutionOverflow(CapturedOutputLimitExceeded),
    Failed(String),
}

struct StreamCapture {
    stream: CapturedStream,
    result: StreamCaptureResult,
}

#[cfg(test)]
pub(in crate::resolution::acquisition) fn run_command_bounded(
    command: ResolverPreparedExecution,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    run_command_bounded_with_budget(
        command,
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
        GitCapturedOutputBudget::new(u64::MAX),
    )
}

pub(in crate::resolution::acquisition) fn run_command_bounded_with_budget(
    command: ResolverPreparedExecution,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    captured_output_budget: GitCapturedOutputBudget,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    run_command_bounded_with_stdin_and_budget(
        command,
        Stdio::null(),
        operation,
        stdout_limit,
        stderr_limit,
        timeout,
        captured_output_budget,
    )
}

pub(in crate::resolution::acquisition) fn run_command_bounded_with_stdin_and_budget(
    mut command: ResolverPreparedExecution,
    stdin: Stdio,
    operation: &str,
    stdout_limit: usize,
    stderr_limit: usize,
    timeout: Duration,
    captured_output_budget: GitCapturedOutputBudget,
) -> Result<BoundedCommandOutput, SourceResolveError> {
    let started = Instant::now();
    let deadline = started.checked_add(timeout).unwrap_or(started);
    let cleanup_reserve = command_cleanup_reserve(timeout);
    let execution_deadline = deadline.checked_sub(cleanup_reserve).unwrap_or(started);
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child =
        ResolverExecutionChild::spawn(command).map_err(|error| SourceResolveError::Git {
            operation: format!("{operation} spawn"),
            status: None,
            stderr: error.to_string(),
        })?;
    let stdout = child.take_stdout().expect("command stdout was piped");
    let stderr = child.take_stderr().expect("command stderr was piped");
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = spawn_stream_capture(
        stdout,
        CapturedStream::Stdout,
        stdout_limit,
        captured_output_budget.clone(),
        &sender,
    ) {
        return fail_after_cleanup_before(
            &mut child,
            operation,
            deadline,
            SourceResolveError::Git {
                operation: format!("{operation} stdout capture"),
                status: None,
                stderr: error.to_string(),
            },
        );
    }
    if let Err(error) = spawn_stream_capture(
        stderr,
        CapturedStream::Stderr,
        stderr_limit,
        captured_output_budget,
        &sender,
    ) {
        return fail_after_cleanup_before(
            &mut child,
            operation,
            deadline,
            SourceResolveError::Git {
                operation: format!("{operation} stderr capture"),
                status: None,
                stderr: error.to_string(),
            },
        );
    }
    drop(sender);

    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if Instant::now() >= deadline {
            return fail_after_cleanup_before(
                &mut child,
                operation,
                deadline,
                SourceResolveError::GitTimedOut {
                    operation: operation.to_owned(),
                    timeout_millis: duration_millis(timeout),
                },
            );
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(Some(status)) => {
                    terminate_child_before(&mut child, operation, deadline)?;
                    Some(status)
                }
                Ok(None) => None,
                Err(error) => {
                    return fail_after_cleanup_before(
                        &mut child,
                        operation,
                        deadline,
                        SourceResolveError::Git {
                            operation: format!("{operation} wait"),
                            status: None,
                            stderr: error.to_string(),
                        },
                    );
                }
            };
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            let completion = child.finish().map_err(|error| {
                SourceResolveError::GitExecutionBoundaryInvalid {
                    message: format!("cannot issue resolver execution completion: {error}"),
                }
            })?;
            return Ok(BoundedCommandOutput {
                status: status.expect("status was checked"),
                stdout: stdout.expect("stdout was checked"),
                stderr: stderr.expect("stderr was checked"),
                completion,
            });
        }

        let now = Instant::now();
        if now >= execution_deadline {
            return fail_after_cleanup_before(
                &mut child,
                operation,
                deadline,
                SourceResolveError::GitTimedOut {
                    operation: operation.to_owned(),
                    timeout_millis: duration_millis(timeout),
                },
            );
        }
        let wait = PROCESS_POLL_INTERVAL.min(execution_deadline.saturating_duration_since(now));
        match receiver.recv_timeout(wait) {
            Ok(capture) => {
                let bytes = match capture.result {
                    StreamCaptureResult::Complete(bytes) => bytes,
                    StreamCaptureResult::Overflow => {
                        return fail_after_cleanup_before(
                            &mut child,
                            operation,
                            deadline,
                            SourceResolveError::GitOutputOverflow {
                                operation: operation.to_owned(),
                                stream: capture.stream.name().to_owned(),
                                limit: match capture.stream {
                                    CapturedStream::Stdout => stdout_limit,
                                    CapturedStream::Stderr => stderr_limit,
                                },
                            },
                        );
                    }
                    StreamCaptureResult::ResolutionOverflow(exceeded) => {
                        return fail_after_cleanup_before(
                            &mut child,
                            operation,
                            deadline,
                            SourceResolveError::GitResolutionCapturedOutputLimit {
                                ceiling: exceeded.ceiling,
                                attempted: exceeded.attempted,
                            },
                        );
                    }
                    StreamCaptureResult::Failed(message) => {
                        return fail_after_cleanup_before(
                            &mut child,
                            operation,
                            deadline,
                            SourceResolveError::Git {
                                operation: format!("{operation} {} capture", capture.stream.name()),
                                status: None,
                                stderr: message,
                            },
                        );
                    }
                };
                match capture.stream {
                    CapturedStream::Stdout => stdout = Some(bytes),
                    CapturedStream::Stderr => stderr = Some(bytes),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if stdout.is_none() || stderr.is_none() {
                    return fail_after_cleanup_before(
                        &mut child,
                        operation,
                        deadline,
                        SourceResolveError::Git {
                            operation: format!("{operation} capture"),
                            status: None,
                            stderr: "output capture ended before both streams completed".to_owned(),
                        },
                    );
                }
                std::thread::sleep(wait);
            }
        }
    }
}

fn spawn_stream_capture<R>(
    reader: R,
    stream: CapturedStream,
    limit: usize,
    captured_output_budget: GitCapturedOutputBudget,
    sender: &mpsc::Sender<StreamCapture>,
) -> std::io::Result<()>
where
    R: Read + Send + 'static,
{
    let sender = sender.clone();
    std::thread::Builder::new()
        .name(format!("omega-git-{}", stream.name()))
        .spawn(move || {
            let result = capture_stream_bounded_with_budget(reader, limit, &captured_output_budget);
            let _ = sender.send(StreamCapture { stream, result });
        })?;
    Ok(())
}

#[cfg(test)]
pub(in crate::resolution::acquisition) fn capture_stream_bounded<R>(
    mut reader: R,
    limit: usize,
) -> StreamCaptureResult
where
    R: Read,
{
    capture_stream_bounded_with_budget(&mut reader, limit, &GitCapturedOutputBudget::new(u64::MAX))
}

fn capture_stream_bounded_with_budget<R>(
    mut reader: R,
    limit: usize,
    captured_output_budget: &GitCapturedOutputBudget,
) -> StreamCaptureResult
where
    R: Read,
{
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        let count = match reader.read(&mut chunk) {
            Ok(0) => return StreamCaptureResult::Complete(bytes),
            Ok(count) => count,
            Err(error) => return StreamCaptureResult::Failed(error.to_string()),
        };
        let Some(next_length) = bytes.len().checked_add(count) else {
            return StreamCaptureResult::Overflow;
        };
        if next_length > limit {
            return StreamCaptureResult::Overflow;
        }
        if let Err(exceeded) = captured_output_budget.charge(count) {
            return StreamCaptureResult::ResolutionOverflow(exceeded);
        }
        if bytes.try_reserve(count).is_err() {
            return StreamCaptureResult::Failed("output capture allocation failed".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

pub(in crate::resolution::acquisition) fn command_cleanup_reserve(timeout: Duration) -> Duration {
    GIT_COMMAND_CLEANUP_TIMEOUT.min(timeout / 4)
}

fn fail_after_cleanup_before<T>(
    child: &mut ResolverExecutionChild,
    operation: &str,
    deadline: Instant,
    original: SourceResolveError,
) -> Result<T, SourceResolveError> {
    match terminate_child_before(child, operation, deadline) {
        Ok(()) => Err(original),
        Err(cleanup) => Err(cleanup),
    }
}

fn terminate_child_before(
    child: &mut ResolverExecutionChild,
    operation: &str,
    command_deadline: Instant,
) -> Result<(), SourceResolveError> {
    child
        .terminate()
        .map_err(|error| SourceResolveError::GitCleanupFailed {
            operation: operation.to_owned(),
            message: format!("could not terminate the process container: {error}"),
        })?;
    let started = Instant::now();
    let cleanup_budget =
        GIT_COMMAND_CLEANUP_TIMEOUT.min(command_deadline.saturating_duration_since(started));
    let cleanup_deadline = started.checked_add(cleanup_budget).unwrap_or(started);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => {
                return Err(SourceResolveError::GitCleanupFailed {
                    operation: operation.to_owned(),
                    message: format!("could not reap the process: {error}"),
                });
            }
        }
        if Instant::now() >= cleanup_deadline {
            let message = format!(
                "could not reap the terminated process container within {} milliseconds",
                duration_millis(cleanup_budget)
            );
            return Err(SourceResolveError::GitCleanupFailed {
                operation: operation.to_owned(),
                message,
            });
        }
        std::thread::sleep(
            PROCESS_POLL_INTERVAL.min(cleanup_deadline.saturating_duration_since(Instant::now())),
        );
    }
}

pub(in crate::resolution::acquisition) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
