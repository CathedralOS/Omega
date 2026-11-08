use super::{
    GitExecutionTransport, ResolverExecutionPhase, sealed_git_command, test_system_git_executor,
};
use std::ffi::{OsStr, OsString};

#[test]
fn git_commands_admit_host_https_ssh_routing_and_close_other_protocols() {
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
                Some(OsString::from("https:ssh")),
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
    assert_eq!(
        command.get_program(),
        executor.execution_backend.executable().as_os_str()
    );
    assert_eq!(command.get_current_dir(), Some(working_directory.as_path()));
    for required in [
        "--no-replace-objects",
        "protocol.allow=never",
        "protocol.ext.allow=never",
        "protocol.http.allow=never",
        "protocol.git.allow=never",
        "protocol.file.allow=never",
        "protocol.https.allow=always",
        "protocol.ssh.allow=always",
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
