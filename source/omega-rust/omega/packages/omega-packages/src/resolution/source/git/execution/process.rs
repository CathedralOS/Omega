//! Sealed command construction, bounded capture, deadlines, and cleanup.

use super::*;

pub(in crate::resolution::source) enum GitCommandStdinIdentity {
    Null,
    ExactBytes { length: u64, identity: String },
}

pub(in crate::resolution::source) fn git_batch_stdin_identity(
    entries: &[GitTreeEntry],
) -> GitCommandStdinIdentity {
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        hasher.update(entry.oid.as_bytes());
        hasher.update(b"\n");
        length = length
            .saturating_add(entry.oid.len() as u64)
            .saturating_add(1);
    }
    GitCommandStdinIdentity::ExactBytes {
        length,
        identity: format_sha256(&hasher.finalize()),
    }
}

pub(in crate::resolution::source) fn git_command_configuration_identity(
    command: &Command,
    phase: ResolverExecutionPhase,
    stdin: &GitCommandStdinIdentity,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"omega-git-command-configuration-v1\0");
    hasher.update([match phase {
        ResolverExecutionPhase::TransportDiscovery => 1,
        ResolverExecutionPhase::RepositoryInitialization => 2,
        ResolverExecutionPhase::Fetch => 3,
        ResolverExecutionPhase::RepositoryInspection => 4,
    }]);
    hash_command_os_str(&mut hasher, command.get_program());
    let arguments = command.get_args().collect::<Vec<_>>();
    hash_length(&mut hasher, arguments.len() as u64);
    for argument in arguments {
        hash_command_os_str(&mut hasher, argument);
    }
    let mut environment = command.get_envs().collect::<Vec<_>>();
    environment.sort_by(|left, right| left.0.cmp(right.0));
    hash_length(&mut hasher, environment.len() as u64);
    for (name, value) in environment {
        hash_command_os_str(&mut hasher, name);
        match value {
            Some(value) => {
                hasher.update([1]);
                hash_command_os_str(&mut hasher, value);
            }
            None => hasher.update([0]),
        }
    }
    match command.get_current_dir() {
        Some(directory) => {
            hasher.update([1]);
            hash_command_os_str(&mut hasher, directory.as_os_str());
        }
        None => hasher.update([0]),
    }
    match stdin {
        GitCommandStdinIdentity::Null => hasher.update([1]),
        GitCommandStdinIdentity::ExactBytes { length, identity } => {
            hasher.update([2]);
            hasher.update(length.to_le_bytes());
            hash_bytes(&mut hasher, identity.as_bytes());
        }
    }
    format_sha256(&hasher.finalize())
}

fn hash_command_os_str(hasher: &mut Sha256, value: &OsStr) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash_bytes(hasher, value.as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = value.encode_wide().collect::<Vec<_>>();
        hash_length(hasher, units.len() as u64);
        for unit in units {
            hasher.update(unit.to_le_bytes());
        }
    }
}

pub(in crate::resolution::source) fn run_git<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<(), SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, phase, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub(in crate::resolution::source) fn run_git_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<String, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, phase, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub(in crate::resolution::source) fn run_git_bytes_stdout<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<Vec<u8>, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = run_git_output(executor, working_directory, phase, args)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(SourceResolveError::Git {
            operation: "command".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub(in crate::resolution::source) fn run_git_output<I, S>(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    args: I,
) -> Result<BoundedCommandOutput, SourceResolveError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let endpoint_route = if matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    ) {
        Some(
            executor
                .execution_backend
                .open_endpoint_route(
                    executor.requested_network_endpoint.clone(),
                    executor.network_transfer_budget.clone(),
                )
                .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: format!("cannot open the compiler-owned endpoint route: {error}"),
                })?,
        )
    } else {
        None
    };
    let mut command =
        sealed_git_command_with_route(executor, working_directory, phase, endpoint_route.as_ref())?;
    let command_timeout = executor.begin_launch()?;
    command.args(args);
    let command_identity =
        git_command_configuration_identity(&command, phase, &GitCommandStdinIdentity::Null);
    let result = run_command_bounded_with_budget(
        &mut command,
        "command",
        GIT_STDOUT_LIMIT,
        GIT_STDERR_LIMIT,
        command_timeout,
        executor.captured_output_budget.clone(),
    );
    let endpoint_result = endpoint_route
        .map(ResolverExecutionEndpointRoute::finish)
        .transpose()
        .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("compiler-owned endpoint route failed: {error}"),
        });
    let endpoint_validation = endpoint_result
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|observation| validate_network_transfer_outcome(observation.as_ref()));
    let output = reconcile_git_command_endpoint_result(
        result,
        endpoint_validation,
        executor.verify(),
        executor.verify_budget(),
    )?;
    let endpoint_observation =
        endpoint_result.expect("successful reconciliation checked endpoint result");
    executor.record_command_execution(phase, command_identity, &output, endpoint_observation)?;
    Ok(output)
}

fn validate_network_transfer_outcome(
    observation: Option<&ResolverExecutionEndpointObservation>,
) -> Result<(), SourceResolveError> {
    let Some(observation) = observation else {
        return Ok(());
    };
    if observation
        .events()
        .iter()
        .any(|event| event.outcome() == ResolverExecutionEndpointOutcome::TransferCeilingReached)
    {
        return Err(SourceResolveError::GitResolutionNetworkTransferCeiling {
            ceiling: observation.route().transfer_byte_ceiling(),
        });
    }
    Ok(())
}

pub(in crate::resolution::source) fn reconcile_git_command_endpoint_result<T>(
    result: Result<T, SourceResolveError>,
    endpoint_result: Result<(), SourceResolveError>,
    executable_result: Result<(), SourceResolveError>,
    budget_result: Result<(), SourceResolveError>,
) -> Result<T, SourceResolveError> {
    match (result, endpoint_result, executable_result, budget_result) {
        (Err(error @ SourceResolveError::GitCleanupFailed { .. }), _, _, _) => Err(error),
        (_, Err(error), _, _) => Err(error),
        (_, _, Err(error), _) => Err(error),
        (_, _, _, Err(error)) => Err(error),
        (result, Ok(()), Ok(()), Ok(())) => result,
    }
}

pub(in crate::resolution::source) fn reconcile_git_command_result<T>(
    result: Result<T, SourceResolveError>,
    executable_result: Result<(), SourceResolveError>,
    budget_result: Result<(), SourceResolveError>,
) -> Result<T, SourceResolveError> {
    match (result, executable_result, budget_result) {
        (Err(error @ SourceResolveError::GitCleanupFailed { .. }), _, _) => Err(error),
        (_, Err(error), _) => Err(error),
        (_, _, Err(error)) => Err(error),
        (result, Ok(()), Ok(())) => result,
    }
}

pub(in crate::resolution::source) fn reconcile_git_cache_operation_result<T>(
    operation_result: Result<T, SourceResolveError>,
    namespace_result: Result<(), SourceResolveError>,
    invalidation_result: Option<Result<(), SourceResolveError>>,
) -> Result<T, SourceResolveError> {
    if let Err(error) = namespace_result {
        return Err(error);
    }
    if let Some(Err(error)) = invalidation_result {
        return Err(error);
    }
    operation_result
}

#[derive(Debug)]
pub(in crate::resolution::source) struct BoundedCommandOutput {
    pub(in crate::resolution::source) status: ExitStatus,
    pub(in crate::resolution::source) stdout: Vec<u8>,
    pub(in crate::resolution::source) stderr: Vec<u8>,
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

pub(in crate::resolution::source) enum StreamCaptureResult {
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
pub(in crate::resolution::source) fn run_command_bounded(
    command: &mut Command,
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

pub(in crate::resolution::source) fn run_command_bounded_with_budget(
    command: &mut Command,
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

pub(in crate::resolution::source) fn run_command_bounded_with_stdin_and_budget(
    command: &mut Command,
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
            return Ok(BoundedCommandOutput {
                status: status.expect("status was checked"),
                stdout: stdout.expect("stdout was checked"),
                stderr: stderr.expect("stderr was checked"),
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
pub(in crate::resolution::source) fn capture_stream_bounded<R>(
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

pub(in crate::resolution::source) fn command_cleanup_reserve(timeout: Duration) -> Duration {
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
    let kill_error = child.kill().err();
    let started = Instant::now();
    let cleanup_budget =
        GIT_COMMAND_CLEANUP_TIMEOUT.min(command_deadline.saturating_duration_since(started));
    let cleanup_deadline = started.checked_add(cleanup_budget).unwrap_or(started);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                if let Some(error) = kill_error
                    .as_ref()
                    .filter(|error| !process_group_already_absent(error))
                {
                    return Err(SourceResolveError::GitCleanupFailed {
                        operation: operation.to_owned(),
                        message: format!("could not terminate the process group: {error}"),
                    });
                }
                return Ok(());
            }
            Ok(None) => {}
            Err(error) => {
                return Err(SourceResolveError::GitCleanupFailed {
                    operation: operation.to_owned(),
                    message: format!("could not reap the process: {error}"),
                });
            }
        }
        if Instant::now() >= cleanup_deadline {
            let message = match &kill_error {
                Some(error) => format!(
                    "could not terminate the process group ({error}) or reap it within {} milliseconds",
                    duration_millis(cleanup_budget)
                ),
                None => format!(
                    "could not reap the terminated process within {} milliseconds",
                    duration_millis(cleanup_budget)
                ),
            };
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

pub(in crate::resolution::source) fn process_group_already_absent(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    {
        // POSIX ESRCH alone proves that no process group exists. EPERM proves
        // the opposite: a group exists but this resolver cannot signal it.
        error.raw_os_error() == Some(3)
    }
    #[cfg(not(unix))]
    {
        error.kind() == std::io::ErrorKind::InvalidInput
    }
}

pub(in crate::resolution::source) fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub(in crate::resolution::source) fn sealed_git_command_with_route(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    endpoint_route: Option<&ResolverExecutionEndpointRoute>,
) -> Result<Command, SourceResolveError> {
    executor.verify()?;
    if !working_directory.is_absolute() {
        return Err(SourceResolveError::Git {
            operation: "command configuration".to_owned(),
            status: None,
            stderr: format!(
                "working directory `{}` is not absolute",
                working_directory.display()
            ),
        });
    }
    let metadata =
        std::fs::metadata(working_directory).map_err(|error| io_error(working_directory, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::NotDirectory {
            path: working_directory.to_path_buf(),
        });
    }

    let network_phase = matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    );
    let mut helper_executables = Vec::new();
    if network_phase && let Some(helper) = &executor.transport_executable {
        helper_executables.push(helper.identity.invocation_path.clone());
        if helper.identity.path != helper.identity.invocation_path {
            helper_executables.push(helper.identity.path.clone());
        }
    }
    if network_phase {
        for helper in &executor.execution_helpers {
            helper_executables.push(helper.identity.invocation_path.clone());
            if helper.identity.path != helper.identity.invocation_path {
                helper_executables.push(helper.identity.path.clone());
            }
        }
    }
    let mutable_root = match phase {
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            Some(working_directory)
        }
        ResolverExecutionPhase::TransportDiscovery
        | ResolverExecutionPhase::RepositoryInspection => None,
    };
    let network_transport =
        network_phase.then(|| executor.execution_transport.resolver_network_transport());
    let command_result = match phase {
        ResolverExecutionPhase::RepositoryInspection => executor
            .execution_backend
            .command_with_inspection_read_root_observation(
                &executor.identity.path,
                &helper_executables,
                working_directory,
            ),
        ResolverExecutionPhase::TransportDiscovery => executor
            .execution_backend
            .command_with_discovery_route_observation(
                &executor.identity.path,
                &helper_executables,
                network_transport.expect("discovery transport derived from the closed phase"),
                endpoint_route.expect("discovery route opened from the validated request"),
                working_directory,
            ),
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            executor
                .execution_backend
                .command_with_endpoint_route_observation(
                    &executor.identity.path,
                    &helper_executables,
                    phase,
                    network_transport,
                    endpoint_route,
                    mutable_root,
                )
        }
    };
    let (mut command, execution_policy_observation) =
        command_result.map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
            message: error.to_string(),
        })?;
    executor
        .execution_policy_observations
        .borrow_mut()
        .push(execution_policy_observation);
    command
        .env_clear()
        .current_dir(working_directory)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", git_helper_path(executor))
        .env(
            "GIT_ALLOW_PROTOCOL",
            executor.execution_transport.allowed_protocol(),
        )
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("--no-replace-objects")
        .arg("-c")
        .arg(format!("core.hooksPath={}", null_device()))
        .args(["-c", "protocol.allow=never"])
        .arg("-c")
        .arg(format!("protocol.file.allow={}", {
            #[cfg(test)]
            {
                executor
                    .execution_transport
                    .permits(GitExecutionTransport::File)
            }
            #[cfg(not(test))]
            {
                "never"
            }
        }))
        .args(["-c", "protocol.http.allow=never"])
        .arg("-c")
        .arg(format!(
            "protocol.https.allow={}",
            executor
                .execution_transport
                .permits(GitExecutionTransport::Https)
        ))
        .arg("-c")
        .arg(format!(
            "protocol.ssh.allow={}",
            executor
                .execution_transport
                .permits(GitExecutionTransport::Ssh)
        ))
        .args([
            "-c",
            "protocol.git.allow=never",
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "http.followRedirects=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "transfer.fsckObjects=true",
            "-c",
            "fetch.recurseSubmodules=false",
            "-c",
            "submodule.recurse=false",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
        ]);
    if network_phase && executor.execution_transport == GitExecutionTransport::Https {
        let helper = executor
            .transport_executable
            .as_ref()
            .expect("validated HTTPS executor retains its transport helper");
        let helper_directory = helper
            .identity
            .invocation_path
            .parent()
            .expect("validated HTTPS helper has an absolute parent");
        command.env("GIT_EXEC_PATH", helper_directory);
        let route = endpoint_route.expect("networked HTTPS command retains an endpoint route");
        command
            .arg("-c")
            .arg(format!("http.proxy={}", route.policy().http_proxy_url()));
    }
    if let Some(transport_executable) = &executor.transport_executable {
        if network_phase && executor.execution_transport == GitExecutionTransport::Ssh {
            let route = endpoint_route.expect("networked SSH command retains an endpoint route");
            let connector = executor
                .resolver_connect_helper()
                .expect("validated SSH executor retains its CONNECT helper");
            let connector_directory = connector
                .identity
                .invocation_path
                .parent()
                .expect("validated CONNECT helper has an absolute parent");
            command
                .env(
                    "GIT_SSH_COMMAND",
                    sealed_ssh_command(&transport_executable.identity.path),
                )
                .env("GIT_SSH_VARIANT", "ssh")
                .env("PATH", connector_directory)
                .env(
                    RESOLVER_CONNECT_BROKER_ENVIRONMENT,
                    route.policy().broker_endpoint().to_string(),
                )
                .env(
                    RESOLVER_CONNECT_TARGET_ENVIRONMENT,
                    route.policy().requested_endpoint().authority(),
                );
        }
    }
    Ok(command)
}

#[cfg(test)]
pub(in crate::resolution::source) fn sealed_git_command(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
) -> Result<Command, SourceResolveError> {
    let route = if matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    ) {
        Some(
            executor
                .execution_backend
                .open_endpoint_route(
                    executor.requested_network_endpoint.clone(),
                    executor.network_transfer_budget.clone(),
                )
                .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: format!("cannot open the compiler-owned endpoint route: {error}"),
                })?,
        )
    } else {
        None
    };
    sealed_git_command_with_route(executor, working_directory, phase, route.as_ref())
}

#[cfg(unix)]
pub(in crate::resolution::source) fn git_helper_path(executor: &GitExecutor) -> OsString {
    if executor.execution_transport == GitExecutionTransport::Https {
        return executor
            .transport_executable
            .as_ref()
            .and_then(|helper| helper.identity.invocation_path.parent())
            .map(Path::as_os_str)
            .map(OsStr::to_os_string)
            .unwrap_or_default();
    }
    OsString::from("/usr/bin:/bin")
}

#[cfg(unix)]
pub(in crate::resolution::source) fn sealed_ssh_command(ssh_executable: &Path) -> OsString {
    OsString::from(format!(
        "{} -F none -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes -oProxyUseFdpass=no -oProxyCommand={}",
        ssh_executable.display(),
        resolver_connect_helper_command_name(),
    ))
}

fn resolver_connect_helper_command_name() -> String {
    if cfg!(windows) {
        format!("{RESOLVER_CONNECT_HELPER_BASENAME}.exe")
    } else {
        RESOLVER_CONNECT_HELPER_BASENAME.to_owned()
    }
}

#[cfg(windows)]
pub(in crate::resolution::source) fn git_helper_path(executor: &GitExecutor) -> OsString {
    if executor.execution_transport == GitExecutionTransport::Https {
        return executor
            .transport_executable
            .as_ref()
            .and_then(|helper| helper.identity.invocation_path.parent())
            .map(Path::as_os_str)
            .map(OsStr::to_os_string)
            .unwrap_or_default();
    }
    let mut directories = Vec::new();
    if let Some(parent) = executor.identity.path.parent() {
        directories.push(parent.to_path_buf());
        if let Some(root) = parent.parent() {
            directories.push(root.join("bin"));
            directories.push(root.join("usr/bin"));
        }
    }
    std::env::join_paths(directories).unwrap_or_default()
}

#[cfg(windows)]
pub(in crate::resolution::source) fn sealed_ssh_command(ssh_executable: &Path) -> OsString {
    OsString::from(format!(
        "\"{}\" -F NUL -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes",
        ssh_executable.display()
    ))
}

#[cfg(unix)]
pub(in crate::resolution::source) fn null_device() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
pub(in crate::resolution::source) fn null_device() -> &'static str {
    "NUL"
}

pub(in crate::resolution::source) fn format_sha256(bytes: &[u8]) -> String {
    format_hex(bytes)
}

pub(in crate::resolution::source) fn format_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
