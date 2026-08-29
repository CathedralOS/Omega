use super::super::{
    GIT_CACHE_REPOSITORY, GitExecutionTransport, LocalSourceLimits,
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_TARGET_ENVIRONMENT,
    ResolverExecutionPhase, SourceResolveError, create_git_source, git_cache_entry_root,
    git_helper_path, local_git_request, null_device, resolve_git_source, run_test_git,
    sealed_git_command, sealed_ssh_command, temp_root, test_system_git_executor,
};
use std::ffi::{OsStr, OsString};
#[cfg(target_os = "macos")]
use std::path::Path;

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
