use super::*;

#[cfg(unix)]
#[test]
fn bounded_command_uses_null_stdin_and_drains_both_streams() {
    let mut null_stdin =
        shell_command("if IFS= read -r value; then printf input; else printf eof; fi");
    let output = run_command_bounded(
        &mut null_stdin,
        "test-null-stdin",
        16,
        16,
        Duration::from_secs(10),
    )
    .expect("null stdin must reach EOF without blocking");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"eof");

    let mut both_streams = shell_command(
        "dd if=/dev/zero bs=65536 count=2 1>&2 2>/dev/null; \
             dd if=/dev/zero bs=65536 count=2 2>/dev/null",
    );
    let output = run_command_bounded(
        &mut both_streams,
        "test-both-streams",
        128 * 1024,
        128 * 1024,
        Duration::from_secs(10),
    )
    .expect("stdout and stderr must be drained concurrently");
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 128 * 1024);
    assert_eq!(output.stderr.len(), 128 * 1024);

    let shared_budget = GitCapturedOutputBudget::new(192 * 1024);
    let mut aggregate_overflow = shell_command(
        "dd if=/dev/zero bs=65536 count=2 1>&2 2>/dev/null; \
             dd if=/dev/zero bs=65536 count=2 2>/dev/null",
    );
    let error = run_command_bounded_with_budget(
        &mut aggregate_overflow,
        "test-shared-output-budget",
        128 * 1024,
        128 * 1024,
        Duration::from_secs(10),
        shared_budget.clone(),
    )
    .expect_err("stdout and stderr must consume one shared cumulative budget");
    assert!(
        matches!(
            error,
            SourceResolveError::GitResolutionCapturedOutputLimit {
                ceiling,
                attempted,
            } if ceiling == 192 * 1024 && attempted > ceiling
        ),
        "unexpected shared-output error: {error:?}"
    );
    assert!(shared_budget.observed() <= shared_budget.ceiling);
}

#[cfg(unix)]
#[test]
fn bounded_command_rejects_stdout_and_stderr_overflow() {
    assert!(matches!(
        capture_stream_bounded(std::io::Cursor::new(vec![0_u8; 1025]), 1024),
        StreamCaptureResult::Overflow
    ));
    for (stream, redirect) in [("stdout", ""), ("stderr", "1>&2")] {
        let script = format!(
            "i=0; while [ $i -lt 4096 ]; do printf x {redirect}; i=$((i + 1)); done; while :; do :; done"
        );
        let mut command = shell_command(&script);
        let error = run_command_bounded(
            &mut command,
            "test-overflow",
            1024,
            1024,
            Duration::from_secs(2),
        )
        .expect_err("capture overflow must fail closed");
        let exact_overflow = matches!(
            &error,
            SourceResolveError::GitOutputOverflow {
                stream: actual,
                limit: 1024,
                ..
            } if actual == stream
        );
        let fail_closed_macos_cleanup = cfg!(target_os = "macos")
            && matches!(&error, SourceResolveError::GitCleanupFailed { .. });
        assert!(
            exact_overflow || fail_closed_macos_cleanup,
            "unexpected overflow error: {error:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn command_deadline_reserves_cleanup_inside_the_same_budget() {
    assert_eq!(command_cleanup_reserve(Duration::ZERO), Duration::ZERO);
    assert_eq!(
        command_cleanup_reserve(Duration::from_millis(50)),
        Duration::from_micros(12_500)
    );
    assert_eq!(
        command_cleanup_reserve(Duration::from_secs(120)),
        GIT_COMMAND_CLEANUP_TIMEOUT
    );
}

#[cfg(unix)]
#[test]
fn bounded_command_terminates_on_deadline() {
    let mut command = shell_command("exec sleep 10");
    let started = Instant::now();
    let error = run_command_bounded(
        &mut command,
        "test-timeout",
        1024,
        1024,
        Duration::from_millis(50),
    )
    .expect_err("deadline must fail closed");
    assert!(matches!(
        error,
        SourceResolveError::GitTimedOut {
            operation,
            timeout_millis: 50,
        } if operation == "test-timeout"
    ));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "timed out subprocess was not terminated promptly"
    );
}

#[cfg(unix)]
#[test]
fn bounded_command_terminates_descendants_on_deadline() {
    let root = temp_root("bounded-descendant-timeout");
    std::fs::create_dir_all(&root).expect("create descendant test root");
    let marker = root.join("survived");
    let mut command = shell_command(
        "(sleep 0.25; printf survived > \"$OMEGA_DESCENDANT_MARKER\") & exec sleep 10",
    );
    command.env("OMEGA_DESCENDANT_MARKER", &marker);

    let error = run_command_bounded(
        &mut command,
        "test-descendant-timeout",
        1024,
        1024,
        Duration::from_millis(50),
    )
    .expect_err("deadline must fail closed and terminate descendants");
    assert!(matches!(error, SourceResolveError::GitTimedOut { .. }));

    std::thread::sleep(Duration::from_millis(400));
    assert!(
        !marker.exists(),
        "a descendant survived the bounded command deadline"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn ssh_transport_executable_reuses_resolver_executable_custody() {
    use std::os::unix::fs::PermissionsExt;

    let temporary_root = temp_root("ssh-transport-executable");
    std::fs::create_dir_all(&temporary_root).expect("create SSH executable custody root");
    let root = temporary_root
        .canonicalize()
        .expect("canonicalize SSH executable custody root");
    let fake_ssh = root.join("ssh");
    std::fs::write(&fake_ssh, b"#!/bin/sh\nexit 0\n").expect("write fake SSH executable");
    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700))
        .expect("make fake SSH executable");

    let executable =
        open_git_transport_executable(&fake_ssh).expect("capture SSH executable identity");
    assert!(executable.identity.path.is_absolute());
    assert_eq!(executable.identity.content_identity.len(), 64);
    verify_git_transport_executable(&executable).expect("verify unchanged SSH executable");

    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o777))
        .expect("make SSH executable unsafe");
    assert!(matches!(
        verify_git_transport_executable(&executable),
        Err(SourceResolveError::GitExecutableChanged { .. })
            | Err(SourceResolveError::GitExecutableInvalid { .. })
    ));

    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn https_transport_executable_binds_invocation_alias_and_canonical_target() {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let temporary_root = temp_root("https-transport-executable");
    std::fs::create_dir_all(&temporary_root).expect("create HTTPS helper custody root");
    let root = temporary_root
        .canonicalize()
        .expect("canonicalize HTTPS helper custody root");
    let bin = root.join("bin");
    let helpers = root.join("libexec/git-core");
    std::fs::create_dir_all(&bin).expect("create fake Git bin directory");
    std::fs::create_dir_all(&helpers).expect("create fake Git helper directory");

    let fake_git = bin.join("git");
    let helper_target = helpers.join("git-remote-http");
    let helper_alias = helpers.join("git-remote-https");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::write(&helper_target, b"#!/bin/sh\nexit 0\n").expect("write fake HTTPS helper target");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    std::fs::set_permissions(&helper_target, std::fs::Permissions::from_mode(0o700))
        .expect("make fake HTTPS helper target executable");
    symlink("git-remote-http", &helper_alias).expect("create HTTPS helper alias");

    let executable = open_https_transport_executable(&fake_git)
        .expect("capture HTTPS helper alias and target identity");
    assert_eq!(executable.identity.invocation_path(), helper_alias);
    assert_eq!(
        executable.identity.path(),
        helper_target
            .canonicalize()
            .expect("canonicalize HTTPS helper target")
    );
    assert_eq!(executable.identity.content_identity().len(), 64);
    verify_git_transport_executable(&executable).expect("verify unchanged HTTPS helper");

    let replacement = helpers.join("replacement");
    std::fs::write(&replacement, b"#!/bin/sh\nexit 1\n").expect("write replacement HTTPS helper");
    std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))
        .expect("make replacement HTTPS helper executable");
    std::fs::remove_file(&helper_alias).expect("remove original HTTPS helper alias");
    symlink("replacement", &helper_alias).expect("replace HTTPS helper alias");
    assert!(matches!(
        verify_git_transport_executable(&executable),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn bounded_command_cleans_up_descendants_after_parent_exit() {
    let mut command = shell_command("(sleep 10) &");
    let started = Instant::now();
    let output = run_command_bounded(
        &mut command,
        "test-descendant-cleanup",
        1024,
        1024,
        Duration::from_secs(2),
    )
    .expect("a completed parent must not wait on descendant-held capture pipes");
    assert!(output.status.success());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "descendant cleanup did not close inherited capture pipes promptly"
    );
}

#[cfg(unix)]
#[test]
fn only_esrch_proves_a_process_group_is_absent() {
    assert!(process_group_already_absent(
        &std::io::Error::from_raw_os_error(3)
    ));
    assert!(!process_group_already_absent(
        &std::io::Error::from_raw_os_error(1)
    ));
}

#[test]
fn cleanup_failure_outranks_whole_resolution_expiry() {
    let result: Result<(), _> = Err(SourceResolveError::GitCleanupFailed {
        operation: "test".to_owned(),
        message: "process group may remain".to_owned(),
    });
    let budget = Err(SourceResolveError::GitResolutionTimedOut { timeout_millis: 1 });

    assert!(matches!(
        reconcile_git_command_result(result, Ok(()), budget),
        Err(SourceResolveError::GitCleanupFailed { .. })
    ));
}

#[test]
fn network_transfer_ceiling_outranks_ordinary_git_failure() {
    let operation = Err(SourceResolveError::Git {
        operation: "command".to_owned(),
        status: Some(1),
        stderr: "connection closed".to_owned(),
    });
    let endpoint = Err(SourceResolveError::GitResolutionNetworkTransferCeiling { ceiling: 1024 });

    assert!(matches!(
        reconcile_git_command_endpoint_result::<()>(operation, endpoint, Ok(()), Ok(())),
        Err(SourceResolveError::GitResolutionNetworkTransferCeiling { ceiling: 1024 })
    ));
}

#[test]
fn git_cache_rejects_local_filter_configuration_without_running_it() {
    let (repo, _) = create_git_source("git-filter-source");
    std::fs::write(repo.join(".gitattributes"), "*.omg filter=omega-test\n")
        .expect("write attributes");
    run_test_git(&repo, ["add", ".gitattributes"]);
    run_test_git(&repo, ["commit", "--quiet", "-m", "declare filter"]);
    let cache = temp_root("git-filter-cache");
    let sentinel = cache.join("filter-ran");
    let request = local_git_request(&repo, "HEAD");
    resolve_git_source(&request, &cache, LocalSourceLimits::default()).expect("prime cache");
    let repository = git_cache_entry_root(&cache, &request).join(GIT_CACHE_REPOSITORY);
    run_test_git(
        &repository,
        [
            "config",
            "--local",
            "filter.omega-test.smudge",
            &format!("touch {}", sentinel.display()),
        ],
    );

    let error = resolve_git_source(&request, &cache, LocalSourceLimits::default())
        .expect_err("local filter configuration must reject");

    assert!(matches!(error, SourceResolveError::GitCacheInvalid { .. }));
    assert!(!sentinel.exists());
    let _ = std::fs::remove_dir_all(&repo);
    let _ = std::fs::remove_dir_all(&cache);
}

#[test]
fn git_commands_seal_ambient_config_protocol_and_execution_injection() {
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
    let helper_directory = executor
        .transport_executable
        .as_ref()
        .expect("HTTPS transport helper")
        .identity
        .invocation_path
        .parent()
        .expect("HTTPS helper parent")
        .to_path_buf();
    let working_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory");
    let command = sealed_git_command(&executor, &working_directory, ResolverExecutionPhase::Fetch)
        .expect("sealed absolute Git command");
    let environment = command
        .get_envs()
        .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
        .collect::<std::collections::BTreeMap<_, _>>();
    let arguments = command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    let expected_environment = std::collections::BTreeMap::from([
        (
            OsString::from("GIT_ALLOW_PROTOCOL"),
            Some(OsString::from("https")),
        ),
        (
            OsString::from("GIT_ATTR_NOSYSTEM"),
            Some(OsString::from("1")),
        ),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            Some(OsString::from(null_device())),
        ),
        (
            OsString::from("GIT_CONFIG_NOSYSTEM"),
            Some(OsString::from("1")),
        ),
        (
            OsString::from("GIT_EXEC_PATH"),
            Some(helper_directory.into_os_string()),
        ),
        (
            OsString::from("GIT_LFS_SKIP_SMUDGE"),
            Some(OsString::from("1")),
        ),
        (
            OsString::from("GIT_NO_LAZY_FETCH"),
            Some(OsString::from("1")),
        ),
        (
            OsString::from("GIT_PROTOCOL_FROM_USER"),
            Some(OsString::from("0")),
        ),
        (
            OsString::from("GIT_TERMINAL_PROMPT"),
            Some(OsString::from("0")),
        ),
        (OsString::from("LANG"), Some(OsString::from("C"))),
        (OsString::from("LC_ALL"), Some(OsString::from("C"))),
        (OsString::from("PATH"), Some(git_helper_path(&executor))),
    ]);
    assert_eq!(environment, expected_environment);
    #[cfg(target_os = "macos")]
    {
        assert_eq!(command.get_program(), OsStr::new("/usr/bin/sandbox-exec"));
        assert!(
            arguments
                .iter()
                .any(|argument| { Path::new(argument) == executor.identity.path.as_path() })
        );
    }
    #[cfg(not(target_os = "macos"))]
    assert_eq!(command.get_program(), executor.identity.path.as_os_str());
    assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "--no-replace-objects")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.allow=never")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.ext.allow=never")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.http.allow=never")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.git.allow=never")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.file.allow=never")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.https.allow=always")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "protocol.ssh.allow=never")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "http.followRedirects=false")
    );
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "fetch.recurseSubmodules=false")
    );
    assert!(arguments.iter().any(|argument| argument == "gc.auto=0"));
    assert!(
        arguments
            .iter()
            .any(|argument| argument == "maintenance.auto=false")
    );
}

#[test]
fn git_commands_admit_only_the_request_transport() {
    let working_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory");
    for (transport, protocol) in [
        (GitExecutionTransport::Https, "https"),
        (GitExecutionTransport::Ssh, "ssh"),
        (GitExecutionTransport::File, "file"),
    ] {
        let executor = test_system_git_executor(transport).expect("system Git executor");
        let command =
            sealed_git_command(&executor, &working_directory, ResolverExecutionPhase::Fetch)
                .expect("sealed absolute Git command");
        let environment = command
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
            .collect::<std::collections::BTreeMap<_, _>>();
        let arguments = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            environment.get(OsStr::new("GIT_ALLOW_PROTOCOL")),
            Some(&Some(OsString::from(protocol)))
        );
        match transport {
            GitExecutionTransport::Https => {
                let helper = executor
                    .transport_executable
                    .as_ref()
                    .expect("HTTPS transport executable identity");
                assert!(helper.identity.invocation_path.is_absolute());
                assert!(helper.identity.path.is_absolute());
                assert_eq!(helper.identity.content_identity.len(), 64);
                let helper_directory = helper.identity.invocation_path.parent().unwrap();
                assert_eq!(
                    environment.get(OsStr::new("GIT_EXEC_PATH")),
                    Some(&Some(helper_directory.as_os_str().to_owned()))
                );
                assert_eq!(
                    environment.get(OsStr::new("PATH")),
                    Some(&Some(helper_directory.as_os_str().to_owned()))
                );
                assert!(!environment.contains_key(OsStr::new("GIT_SSH_COMMAND")));
                assert!(!environment.contains_key(OsStr::new("GIT_SSH_VARIANT")));
                assert!(!environment.contains_key(OsStr::new(RESOLVER_CONNECT_BROKER_ENVIRONMENT)));
                assert!(!environment.contains_key(OsStr::new(RESOLVER_CONNECT_TARGET_ENVIRONMENT)));
                assert!(
                    arguments
                        .iter()
                        .any(|argument| { argument.starts_with("http.proxy=http://127.0.0.1:") })
                );
            }
            GitExecutionTransport::Ssh => {
                let transport_executable = executor
                    .transport_executable
                    .as_ref()
                    .expect("SSH transport executable identity");
                assert!(transport_executable.identity.path.is_absolute());
                assert_eq!(transport_executable.identity.content_identity.len(), 64);
                assert_eq!(
                    environment.get(OsStr::new("GIT_SSH_COMMAND")),
                    Some(&Some(sealed_ssh_command(
                        &transport_executable.identity.path
                    )))
                );
                assert_eq!(
                    environment.get(OsStr::new("GIT_SSH_VARIANT")),
                    Some(&Some(OsString::from("ssh")))
                );
                let connector = executor
                    .resolver_connect_helper()
                    .expect("SSH CONNECT helper identity");
                assert_eq!(
                    environment.get(OsStr::new("PATH")),
                    Some(&Some(
                        connector
                            .identity
                            .invocation_path
                            .parent()
                            .expect("CONNECT helper parent")
                            .as_os_str()
                            .to_owned()
                    ))
                );
                assert_eq!(
                    environment.get(OsStr::new(RESOLVER_CONNECT_TARGET_ENVIRONMENT)),
                    Some(&Some(OsString::from("127.0.0.1:9")))
                );
                assert!(
                    environment
                        .get(OsStr::new(RESOLVER_CONNECT_BROKER_ENVIRONMENT))
                        .and_then(Option::as_ref)
                        .is_some_and(|endpoint| endpoint
                            .to_string_lossy()
                            .starts_with("127.0.0.1:"))
                );
                assert!(
                    !arguments
                        .iter()
                        .any(|argument| argument.starts_with("http.proxy="))
                );
                assert!(!environment.contains_key(OsStr::new("GIT_EXEC_PATH")));
            }
            GitExecutionTransport::File => {
                assert!(!environment.contains_key(OsStr::new("GIT_SSH_COMMAND")));
                assert!(!environment.contains_key(OsStr::new("GIT_SSH_VARIANT")));
                assert!(!environment.contains_key(OsStr::new("GIT_EXEC_PATH")));
                assert!(executor.transport_executable.is_none());
                assert!(
                    !arguments
                        .iter()
                        .any(|argument| argument.starts_with("http.proxy="))
                );
            }
        }
        for (configured, candidate) in [
            ("file", GitExecutionTransport::File),
            ("https", GitExecutionTransport::Https),
            ("ssh", GitExecutionTransport::Ssh),
        ] {
            let expected = format!(
                "protocol.{configured}.allow={}",
                transport.permits(candidate)
            );
            assert!(
                arguments.iter().any(|argument| argument == &expected),
                "missing {expected:?} for {transport:?}"
            );
        }
    }
}

#[cfg(target_os = "macos")]
fn loopback_acceptance_canary() -> (u16, std::thread::JoinHandle<()>) {
    use std::io::ErrorKind;
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind helper-chain canary");
    listener
        .set_nonblocking(true)
        .expect("make helper-chain canary nonblocking");
    let port = listener.local_addr().expect("read canary address").port();
    let acceptance = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            match listener.accept() {
                Ok((_connection, _address)) => return,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "resolver helper chain did not reach the loopback listener"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("helper-chain listener failed: {error}"),
            }
        }
    });
    (port, acceptance)
}

#[cfg(target_os = "macos")]
fn bounded_system_executor(transport: GitExecutionTransport, port: u16) -> GitExecutor {
    let executable = system_git_candidates()
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .expect("find concrete system Git");
    GitExecutor::open_with_budget_for_transport(
        executable,
        1,
        Duration::from_secs(3),
        git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
        git_resolution_network_transfer_ceiling(LocalSourceLimits::default()),
        transport,
        ResolverExecutionRequestedEndpoint::new("127.0.0.1", port)
            .expect("construct loopback execution endpoint"),
    )
    .expect("open bounded system Git executor")
}

#[cfg(target_os = "macos")]
#[test]
fn macos_https_execution_chain_reaches_the_selected_endpoint() {
    let (port, acceptance) = loopback_acceptance_canary();
    let executor = bounded_system_executor(GitExecutionTransport::Https, port);
    let working_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory");
    let locator = format!("https://127.0.0.1:{port}/repository");
    let output = run_git_output(
        &executor,
        &working_directory,
        ResolverExecutionPhase::TransportDiscovery,
        [OsStr::new("ls-remote"), OsStr::new(&locator)],
    )
    .expect("launch the retained HTTPS executable chain");
    assert!(!output.status.success());
    acceptance.join().expect("observe HTTPS helper connection");
    let transfer = executor
        .network_transfer_observation()
        .expect("reconcile HTTPS transfer accounting");
    assert!(transfer.uploaded() > 0 || transfer.downloaded() > 0);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_ssh_execution_chain_reaches_the_selected_endpoint() {
    if resolver_connect_helper_path()
        .ok()
        .and_then(|path| path.file_name().map(OsStr::to_owned))
        != Some(OsString::from(RESOLVER_CONNECT_HELPER_BASENAME))
    {
        return;
    }
    let (port, acceptance) = loopback_acceptance_canary();
    let executor = bounded_system_executor(GitExecutionTransport::Ssh, port);
    let working_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory");
    let locator = format!("ssh://127.0.0.1:{port}/repository");
    let output = run_git_output(
        &executor,
        &working_directory,
        ResolverExecutionPhase::TransportDiscovery,
        [OsStr::new("ls-remote"), OsStr::new(&locator)],
    )
    .expect("launch the retained SSH executable chain");
    assert!(!output.status.success());
    acceptance.join().expect("observe SSH helper connection");
}

#[cfg(unix)]
#[test]
fn git_executor_uses_committed_absolute_program_cleared_environment_and_explicit_cwd() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("sealed-git-executor");
    let working_directory = root.join("working");
    std::fs::create_dir_all(&working_directory).expect("create explicit working directory");
    let working_directory = working_directory
        .canonicalize()
        .expect("canonical explicit working directory");
    let fake_git = root.join("git");
    std::fs::write(
            &fake_git,
            b"#!/bin/sh\nprintf 'cwd='\npwd\nprintf 'home=%s\\n' \"${HOME-unset}\"\nprintf 'path=%s\\n' \"$PATH\"\n",
        )
        .expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    let executor = GitExecutor::open(&fake_git).expect("capture fake Git identity");

    let output = run_git_output(
        &executor,
        &working_directory,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("ignored-by-test-helper")],
    )
    .expect("run sealed fake Git");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("test helper emits UTF-8");
    assert!(
        stdout.contains(&format!("cwd={}\n", working_directory.display())),
        "sealed helper reported {stdout:?}"
    );
    assert!(stdout.contains("home=unset\n"));
    assert!(stdout.contains("path=/usr/bin:/bin\n"));

    let command = sealed_git_command(&executor, &working_directory, ResolverExecutionPhase::Fetch)
        .expect("construct sealed fake Git command");
    #[cfg(target_os = "macos")]
    {
        assert_eq!(command.get_program(), OsStr::new("/usr/bin/sandbox-exec"));
        assert!(
            command
                .get_args()
                .any(|argument| argument == fake_git.canonicalize().unwrap().as_os_str())
        );
    }
    #[cfg(not(target_os = "macos"))]
    assert_eq!(command.get_program(), fake_git.canonicalize().unwrap());
    assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_rejects_relative_paths_and_executable_drift() {
    use std::os::unix::fs::PermissionsExt;

    assert!(matches!(
        GitExecutor::open(Path::new("git")),
        Err(SourceResolveError::GitExecutableInvalid { .. })
    ));

    let root = temp_root("git-executable-drift");
    std::fs::create_dir_all(&root).expect("create executable drift root");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable");
    let executor = GitExecutor::open(&fake_git).expect("capture fake Git identity");
    let replacement = root.join("replacement");
    std::fs::write(&replacement, b"#!/bin/sh\nexit 1\n").expect("write replacement Git executable");
    std::fs::rename(&replacement, &fake_git).expect("replace fake Git executable");

    assert!(matches!(
        executor.verify(),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));
    assert!(matches!(
        sealed_git_command(&executor, &root, ResolverExecutionPhase::Fetch),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn git_executor_rejects_unsafe_executable_modes_and_ancestry() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-custody");
    std::fs::create_dir_all(&root).expect("create executable custody root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("make executable custody root private");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");

    for unsafe_mode in [0o720, 0o4700, 0o600] {
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(unsafe_mode))
            .expect("set unsafe Git executable mode");
        assert!(matches!(
            GitExecutor::open(&fake_git),
            Err(SourceResolveError::GitExecutableInvalid { .. })
        ));
    }

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore safe Git executable mode");
    let executor = GitExecutor::open(&fake_git).expect("capture safe Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o720))
        .expect("make captured Git executable externally writable");
    assert!(matches!(
        executor.verify(),
        Err(SourceResolveError::GitExecutableChanged { .. })
    ));

    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("restore Git executable before ancestry check");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o720))
        .expect("make Git executable ancestry externally writable");
    assert!(matches!(
        GitExecutor::open(&fake_git),
        Err(SourceResolveError::GitExecutableInvalid { .. })
    ));

    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("restore executable custody root");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(target_os = "macos")]
#[test]
fn git_executor_rejects_extended_acl_allow_entries_on_executable_and_ancestry() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_root("git-executable-acl-custody");
    std::fs::create_dir_all(&root).expect("create executable ACL custody root");
    let root = root
        .canonicalize()
        .expect("canonicalize executable ACL custody root");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("make executable ACL custody root private");
    let fake_git = root.join("git");
    std::fs::write(&fake_git, b"#!/bin/sh\nexit 0\n").expect("write fake Git executable");
    std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Git executable private");

    let executor = GitExecutor::open(&fake_git).expect("capture ACL-free Git executable");
    change_macos_acl(&fake_git, &["+a", "everyone allow write"]);
    let executable_acl_error = executor
        .verify()
        .expect_err("extended ACL allow on executable must reject");
    assert!(
        matches!(
            &executable_acl_error,
            SourceResolveError::GitExecutableInvalid { path, message }
                if path == &fake_git && message.contains("extended ACL allow")
        ),
        "unexpected executable ACL error: {executable_acl_error:?}"
    );
    change_macos_acl(&fake_git, &["-N"]);
    executor
        .verify()
        .expect("removing executable ACL should restore custody");
    change_macos_acl(&fake_git, &["+a", "everyone deny write"]);
    executor
        .verify()
        .expect("deny-only executable ACL does not broaden custody");
    change_macos_acl(&fake_git, &["-N"]);

    change_macos_acl(&root, &["+a", "everyone allow write"]);
    let ancestry_acl_error = executor
        .verify()
        .expect_err("extended ACL allow on ancestry must reject");
    assert!(
        matches!(
            &ancestry_acl_error,
            SourceResolveError::GitExecutableInvalid { path, message }
                if path == &root && message.contains("extended ACL allow")
        ),
        "unexpected ancestry ACL error: {ancestry_acl_error:?}"
    );
    change_macos_acl(&root, &["-N"]);
    executor
        .verify()
        .expect("removing ancestry ACL should restore custody");

    std::fs::remove_file(&fake_git).expect("remove fake Git executable");
    std::fs::remove_dir(&root).expect("remove executable ACL custody root");
}

#[cfg(target_os = "macos")]
#[test]
fn executable_acl_handle_open_rejects_classified_path_replacement() {
    use std::os::unix::fs::PermissionsExt;

    let temporary_root = temp_root("git-executable-acl-handle-replacement");
    std::fs::create_dir_all(&temporary_root).expect("create executable ACL test root");
    let root = temporary_root
        .canonicalize()
        .expect("canonicalize executable ACL test root");
    let executable = root.join("git");
    let retained = root.join("retained");
    std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("write classified executable");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .expect("make classified executable private");
    let classified =
        std::fs::symlink_metadata(&executable).expect("classify executable before replacement");

    std::fs::rename(&executable, &retained).expect("relocate classified executable");
    std::fs::write(&executable, b"#!/bin/sh\nexit 1\n").expect("write replacement executable");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .expect("make replacement executable private");
    change_macos_acl(&executable, &["+a", "everyone allow write"]);

    assert!(matches!(
        verify_macos_open_executable_acl_custody(&executable, &classified),
        Err(SourceResolveError::GitExecutableChanged { path }) if path == executable
    ));

    change_macos_acl(&executable, &["-N"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(target_os = "macos")]
#[test]
fn system_git_executor_excludes_the_apple_dispatcher() {
    let executor = test_system_git_executor(GitExecutionTransport::Https)
        .expect("concrete macOS Git executor");
    assert_ne!(executor.identity.path, Path::new("/usr/bin/git"));
    assert!(executor.identity.path.is_absolute());
}

#[cfg(unix)]
#[test]
fn git_executor_post_check_overrides_success_and_nonzero_exit_after_drift() {
    use std::os::unix::fs::PermissionsExt;

    for exit_status in [0, 7] {
        let root = temp_root(&format!("git-post-drift-{exit_status}"));
        std::fs::create_dir_all(&root).expect("create post-drift root");
        let fake_git = root.join("git");
        let replacement = root.join("git.replacement");
        std::fs::write(
            &fake_git,
            format!("#!/bin/sh\nmv \"$0.replacement\" \"$0\"\nexit {exit_status}\n"),
        )
        .expect("write self-replacing Git executable");
        std::fs::write(&replacement, b"#!/bin/sh\nexit 0\n")
            .expect("write replacement Git executable");
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o700))
            .expect("make self-replacing Git executable");
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o700))
            .expect("make replacement Git executable");
        let executor = GitExecutor::open(&fake_git).expect("capture original Git identity");

        assert!(matches!(
            run_git_output(
                &executor,
                &root,
                ResolverExecutionPhase::Fetch,
                [OsStr::new("ignored")],
            ),
            Err(SourceResolveError::GitExecutableChanged { .. })
        ));

        let _ = std::fs::remove_dir_all(root);
    }
}

#[cfg(unix)]
#[test]
fn git_executor_enforces_whole_resolution_launch_and_time_budgets() {
    use std::os::unix::fs::PermissionsExt;

    assert_eq!(
        git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
        LocalSourceLimits::default().max_bytes + GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE
    );
    assert_eq!(
        git_resolution_captured_output_ceiling(LocalSourceLimits {
            max_bytes: SOURCE_BYTE_ABSOLUTE_LIMIT,
            ..LocalSourceLimits::default()
        }),
        GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT
    );
    assert_eq!(
        git_resolution_network_transfer_ceiling(LocalSourceLimits::default()),
        LocalSourceLimits::default().max_bytes + GIT_NETWORK_TRANSFER_FIXED_ALLOWANCE
    );
    assert_eq!(
        git_resolution_network_transfer_ceiling(LocalSourceLimits {
            max_bytes: SOURCE_BYTE_ABSOLUTE_LIMIT,
            ..LocalSourceLimits::default()
        }),
        GIT_NETWORK_TRANSFER_ABSOLUTE_LIMIT
    );

    let root = temp_root("git-resolution-budget");
    std::fs::create_dir_all(&root).expect("create Git budget root");
    let fast_git = root.join("fast-git");
    std::fs::write(&fast_git, b"#!/bin/sh\nexit 0\n").expect("write fast fake Git");
    std::fs::set_permissions(&fast_git, std::fs::Permissions::from_mode(0o700))
        .expect("make fast fake Git executable");
    let launch_bounded = GitExecutor::open_with_budget(&fast_git, 1, Duration::from_secs(1))
        .expect("capture launch-bounded Git");
    run_git_output(
        &launch_bounded,
        &root,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("first")],
    )
    .expect("first launch fits the budget");
    assert!(matches!(
        run_git_output(
            &launch_bounded,
            &root,
            ResolverExecutionPhase::Fetch,
            [OsStr::new("second")],
        ),
        Err(SourceResolveError::GitResolutionCommandLimit { limit: 1 })
    ));

    let slow_git = root.join("slow-git");
    std::fs::write(&slow_git, b"#!/bin/sh\nsleep 1\n").expect("write slow fake Git");
    std::fs::set_permissions(&slow_git, std::fs::Permissions::from_mode(0o700))
        .expect("make slow fake Git executable");
    let time_bounded = GitExecutor::open_with_budget(&slow_git, 1, Duration::from_millis(30))
        .expect("capture time-bounded Git");
    assert!(matches!(
        run_git_output(
            &time_bounded,
            &root,
            ResolverExecutionPhase::Fetch,
            [OsStr::new("slow")],
        ),
        Err(SourceResolveError::GitResolutionTimedOut { .. })
    ));

    let output_git = root.join("output-git");
    std::fs::write(
            &output_git,
            b"#!/bin/sh\nfor argument do last=$argument; done\nprintf 12345678\nif [ \"$last\" = second ]; then while :; do :; done; fi\n",
        )
        .expect("write output fake Git");
    std::fs::set_permissions(&output_git, std::fs::Permissions::from_mode(0o700))
        .expect("make output fake Git executable");
    let output_bounded =
        GitExecutor::open_with_resource_budgets(&output_git, 2, Duration::from_secs(1), 12)
            .expect("capture output-bounded Git");
    run_git_output(
        &output_bounded,
        &root,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("first")],
    )
    .expect("first command fits cumulative output budget");
    let output_error = run_git_output(
        &output_bounded,
        &root,
        ResolverExecutionPhase::Fetch,
        [OsStr::new("second")],
    )
    .expect_err("second command must exhaust cumulative output budget");
    assert!(
        matches!(
        &output_error,
        SourceResolveError::GitResolutionCapturedOutputLimit {
            ceiling: 12,
            attempted,
        } if *attempted > 12
        ),
        "unexpected cumulative output error: {output_error:?}"
    );

    let _ = std::fs::remove_dir_all(root);
}
