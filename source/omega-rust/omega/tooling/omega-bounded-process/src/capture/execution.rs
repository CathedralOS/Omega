use super::{
    BoundedCaptureBudget, BoundedCaptureBudgetExceeded, BoundedCaptureLimits, BoundedProcessInput,
    BoundedProcessOutput, BoundedProcessRunError, BoundedProcessStream,
};
use crate::{BoundedProcessChild, BoundedProcessPrepared};
use std::io::{Read, Write};
use std::process::ChildStdin;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

enum StreamCaptureResult {
    Complete(Vec<u8>),
    Overflow,
    AggregateOverflow(BoundedCaptureBudgetExceeded),
    Failed(String),
}

struct StreamCapture {
    stream: BoundedProcessStream,
    result: StreamCaptureResult,
}

enum WorkerResult {
    Stream(StreamCapture),
    Input(Result<(), String>),
}

pub(super) fn execute(
    mut prepared: BoundedProcessPrepared,
    input: BoundedProcessInput,
    limits: BoundedCaptureLimits,
    captured_output_budget: BoundedCaptureBudget,
) -> Result<BoundedProcessOutput, BoundedProcessRunError> {
    validate_limits(limits)?;
    let started = Instant::now();
    let deadline = started.checked_add(limits.timeout).unwrap_or(started);
    let cleanup_reserve = limits.cleanup_timeout.min(limits.timeout / 4);
    let execution_deadline = deadline.checked_sub(cleanup_reserve).unwrap_or(started);
    let input_is_piped = !matches!(&input, BoundedProcessInput::Null);
    if input_is_piped {
        prepared.stdin_piped();
    } else {
        prepared.stdin_null();
    }
    prepared.stdout_piped().stderr_piped();

    let mut child = BoundedProcessChild::spawn(prepared)
        .map_err(|error| BoundedProcessRunError::Spawn(error.to_string()))?;
    let stdout = child.take_stdout().expect("bounded stdout was piped");
    let stderr = child.take_stderr().expect("bounded stderr was piped");
    let child_stdin = input_is_piped.then(|| {
        child
            .take_stdin()
            .expect("bounded stdin was piped for non-null input")
    });
    let (sender, receiver) = mpsc::channel();

    spawn_stream_capture(
        stdout,
        BoundedProcessStream::Stdout,
        limits.stdout_bytes,
        captured_output_budget.clone(),
        &sender,
    )
    .map_err(|error| BoundedProcessRunError::WorkerSpawn {
        worker: "stdout capture",
        message: error.to_string(),
    })
    .or_else(|error| fail_after_cleanup(&mut child, deadline, limits, error))?;
    spawn_stream_capture(
        stderr,
        BoundedProcessStream::Stderr,
        limits.stderr_bytes,
        captured_output_budget,
        &sender,
    )
    .map_err(|error| BoundedProcessRunError::WorkerSpawn {
        worker: "stderr capture",
        message: error.to_string(),
    })
    .or_else(|error| fail_after_cleanup(&mut child, deadline, limits, error))?;
    if let Some(child_stdin) = child_stdin {
        spawn_input_transfer(input, child_stdin, &sender)
            .map_err(|error| BoundedProcessRunError::WorkerSpawn {
                worker: "stdin transfer",
                message: error.to_string(),
            })
            .or_else(|error| fail_after_cleanup(&mut child, deadline, limits, error))?;
    }
    drop(sender);

    let mut status_complete = false;
    let mut stdout = None;
    let mut stderr = None;
    let mut input_complete = !input_is_piped;
    loop {
        if Instant::now() >= deadline {
            return fail_after_cleanup(
                &mut child,
                deadline,
                limits,
                BoundedProcessRunError::TimedOut {
                    timeout: limits.timeout,
                },
            );
        }
        if !status_complete {
            status_complete = match child.try_wait() {
                Ok(Some(_)) => {
                    terminate_before(&mut child, deadline, limits)?;
                    true
                }
                Ok(None) => false,
                Err(error) => {
                    return fail_after_cleanup(
                        &mut child,
                        deadline,
                        limits,
                        BoundedProcessRunError::Wait(error.to_string()),
                    );
                }
            };
        }
        if status_complete && stdout.is_some() && stderr.is_some() && input_complete {
            let completion = child
                .finish()
                .map_err(|error| BoundedProcessRunError::Finalize(error.to_string()))?;
            return Ok(BoundedProcessOutput {
                status: completion.status(),
                stdout: stdout.expect("stdout completion was checked"),
                stderr: stderr.expect("stderr completion was checked"),
            });
        }

        let now = Instant::now();
        if now >= execution_deadline {
            return fail_after_cleanup(
                &mut child,
                deadline,
                limits,
                BoundedProcessRunError::TimedOut {
                    timeout: limits.timeout,
                },
            );
        }
        let wait = limits
            .poll_interval
            .min(execution_deadline.saturating_duration_since(now));
        match receiver.recv_timeout(wait) {
            Ok(WorkerResult::Stream(capture)) => {
                let bytes = match capture.result {
                    StreamCaptureResult::Complete(bytes) => bytes,
                    StreamCaptureResult::Overflow => {
                        let limit = match capture.stream {
                            BoundedProcessStream::Stdout => limits.stdout_bytes,
                            BoundedProcessStream::Stderr => limits.stderr_bytes,
                        };
                        return fail_after_cleanup(
                            &mut child,
                            deadline,
                            limits,
                            BoundedProcessRunError::OutputOverflow {
                                stream: capture.stream,
                                limit,
                            },
                        );
                    }
                    StreamCaptureResult::AggregateOverflow(exceeded) => {
                        return fail_after_cleanup(
                            &mut child,
                            deadline,
                            limits,
                            BoundedProcessRunError::AggregateOutputOverflow {
                                ceiling: exceeded.ceiling(),
                                attempted: exceeded.attempted(),
                            },
                        );
                    }
                    StreamCaptureResult::Failed(message) => {
                        return fail_after_cleanup(
                            &mut child,
                            deadline,
                            limits,
                            BoundedProcessRunError::StreamCapture {
                                stream: capture.stream,
                                message,
                            },
                        );
                    }
                };
                match capture.stream {
                    BoundedProcessStream::Stdout => stdout = Some(bytes),
                    BoundedProcessStream::Stderr => stderr = Some(bytes),
                }
            }
            Ok(WorkerResult::Input(result)) => match result {
                Ok(()) => input_complete = true,
                Err(message) => {
                    return fail_after_cleanup(
                        &mut child,
                        deadline,
                        limits,
                        BoundedProcessRunError::InputTransfer(message),
                    );
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if stdout.is_none() || stderr.is_none() || !input_complete {
                    return fail_after_cleanup(
                        &mut child,
                        deadline,
                        limits,
                        BoundedProcessRunError::WorkersEndedEarly,
                    );
                }
                std::thread::sleep(wait);
            }
        }
    }
}

fn validate_limits(limits: BoundedCaptureLimits) -> Result<(), BoundedProcessRunError> {
    if limits.timeout.is_zero()
        || limits.cleanup_timeout.is_zero()
        || limits.poll_interval.is_zero()
    {
        return Err(BoundedProcessRunError::InvalidLimits);
    }
    Ok(())
}

fn spawn_stream_capture<R>(
    reader: R,
    stream: BoundedProcessStream,
    limit: usize,
    budget: BoundedCaptureBudget,
    sender: &mpsc::Sender<WorkerResult>,
) -> std::io::Result<()>
where
    R: Read + Send + 'static,
{
    let sender = sender.clone();
    std::thread::Builder::new()
        .name(format!("omega-bounded-{}", stream.name()))
        .spawn(move || {
            let result = capture_stream(reader, limit, &budget);
            let _ = sender.send(WorkerResult::Stream(StreamCapture { stream, result }));
        })?;
    Ok(())
}

fn spawn_input_transfer(
    input: BoundedProcessInput,
    child_stdin: ChildStdin,
    sender: &mpsc::Sender<WorkerResult>,
) -> std::io::Result<()> {
    let sender = sender.clone();
    std::thread::Builder::new()
        .name("omega-bounded-stdin".to_owned())
        .spawn(move || {
            let result = transfer_input(input, child_stdin).map_err(|error| error.to_string());
            let _ = sender.send(WorkerResult::Input(result));
        })?;
    Ok(())
}

fn transfer_input(input: BoundedProcessInput, mut child_stdin: ChildStdin) -> std::io::Result<()> {
    match input {
        BoundedProcessInput::Null => unreachable!("null input is never transferred"),
        BoundedProcessInput::Bytes(bytes) => child_stdin.write_all(&bytes)?,
        BoundedProcessInput::File(mut file) => {
            std::io::copy(&mut file, &mut child_stdin)?;
        }
    }
    child_stdin.flush()
}

fn capture_stream<R>(
    mut reader: R,
    limit: usize,
    budget: &BoundedCaptureBudget,
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
        if let Err(exceeded) = budget.charge(count) {
            return StreamCaptureResult::AggregateOverflow(exceeded);
        }
        if bytes.try_reserve(count).is_err() {
            return StreamCaptureResult::Failed("output capture allocation failed".to_owned());
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

fn fail_after_cleanup<T>(
    child: &mut BoundedProcessChild,
    deadline: Instant,
    limits: BoundedCaptureLimits,
    original: BoundedProcessRunError,
) -> Result<T, BoundedProcessRunError> {
    match terminate_before(child, deadline, limits) {
        Ok(()) => Err(original),
        Err(cleanup) => Err(cleanup),
    }
}

fn terminate_before(
    child: &mut BoundedProcessChild,
    deadline: Instant,
    limits: BoundedCaptureLimits,
) -> Result<(), BoundedProcessRunError> {
    child.terminate().map_err(|error| {
        BoundedProcessRunError::Cleanup(format!(
            "could not terminate the process container: {error}"
        ))
    })?;
    let started = Instant::now();
    let cleanup_budget = limits
        .cleanup_timeout
        .min(deadline.saturating_duration_since(started));
    let cleanup_deadline = started.checked_add(cleanup_budget).unwrap_or(started);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Ok(()),
            Ok(None) => {}
            Err(error) => {
                return Err(BoundedProcessRunError::Cleanup(format!(
                    "could not reap the process: {error}"
                )));
            }
        }
        if Instant::now() >= cleanup_deadline {
            return Err(BoundedProcessRunError::Cleanup(format!(
                "could not reap the terminated process container within {} milliseconds",
                cleanup_budget.as_millis()
            )));
        }
        std::thread::sleep(
            limits
                .poll_interval
                .min(cleanup_deadline.saturating_duration_since(Instant::now())),
        );
    }
}
