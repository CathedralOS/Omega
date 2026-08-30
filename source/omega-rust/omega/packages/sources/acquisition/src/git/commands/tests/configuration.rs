use super::{
    GIT_CACHE_REPOSITORY, GitExecutionTransport, LocalSourceLimits, ResolverExecutionPhase,
    SourceResolveError, create_git_source, git_cache_entry_root, local_git_request,
    resolve_git_source, run_test_git, sealed_git_command, temp_root, test_system_git_executor,
};
use std::ffi::{OsStr, OsString};

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
fn git_commands_close_package_protocols_without_overriding_host_transport() {
    let executor =
        test_system_git_executor(GitExecutionTransport::Https).expect("system Git executor");
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

    assert_eq!(
        environment,
        std::collections::BTreeMap::from([
            (
                OsString::from("GIT_ALLOW_PROTOCOL"),
                Some(OsString::from("https")),
            ),
            (
                OsString::from("GIT_ATTR_NOSYSTEM"),
                Some(OsString::from("1")),
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
        ])
    );
    assert_eq!(command.get_program(), executor.identity.path.as_os_str());
    assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));
    for required in [
        "--no-replace-objects",
        "protocol.allow=never",
        "protocol.ext.allow=never",
        "protocol.http.allow=never",
        "protocol.git.allow=never",
        "protocol.file.allow=never",
        "protocol.https.allow=always",
        "protocol.ssh.allow=never",
        "http.followRedirects=false",
        "fetch.recurseSubmodules=false",
        "gc.auto=0",
        "maintenance.auto=false",
    ] {
        assert!(
            arguments.iter().any(|argument| argument == required),
            "missing compiler-owned Git policy argument {required:?}"
        );
    }
}

#[test]
fn each_request_transport_keeps_host_routing_and_closes_other_protocols() {
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
        for forbidden in [
            "GIT_EXEC_PATH",
            "GIT_SSH_COMMAND",
            "GIT_SSH_VARIANT",
            "PATH",
            "OMEGA_RESOLVER_CONNECT_BROKER",
            "OMEGA_RESOLVER_CONNECT_TARGET",
        ] {
            assert!(
                !environment.contains_key(OsStr::new(forbidden)),
                "host-routed command must not override {forbidden}"
            );
        }
        assert!(
            !arguments
                .iter()
                .any(|argument| argument.starts_with("http.proxy="))
        );
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
