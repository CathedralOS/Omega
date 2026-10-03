use super::fixture::{Fixture, assert_status};

#[cfg(unix)]
#[test]
fn offline_unrecorded_git_fails_through_every_supported_command_without_transport() {
    use std::process::Command;
    let fixture = Fixture::new();
    let script = fixture.path("transport.sh");
    let log = fixture.path("transport.log");
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf 'unexpected\\n' >> {}\nexit 1\n",
            quote(&log)
        ),
    )
    .unwrap();
    let omega = |arguments: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(fixture.path("root"))
            .env("GIT_SSH_COMMAND", format!("sh {}", quote(&script)))
            .env("GIT_SSH_VARIANT", "simple")
            .args(arguments)
            .output()
            .unwrap()
    };
    let locator = "git@offline-command.invalid:library.git";
    let before = fixture.accepted_files();
    let output = omega(&["install", locator, "--offline"]);
    assert_status(&output, 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("offline resolution"));
    assert_eq!(fixture.accepted_files(), before);
    fixture.write("root/build.omg", &format!("machine build(builder: &mut Build) {{ Build::package(builder, \"cli_project\"); Build::depend(builder, Source::Git {{ repository: \"{locator}\", revision: \"HEAD\" }}); }}\n"));
    let before = fixture.accepted_files();
    for arguments in [
        vec!["--check", "--offline", "main.omg"],
        vec!["update", "--offline"],
        vec!["audit", "packages", "--offline"],
    ] {
        let output = omega(&arguments);
        assert_status(&output, 1);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(text.contains("offline resolution"), "{arguments:?}: {text}");
        assert_eq!(fixture.accepted_files(), before);
        assert!(!fixture.path("root/build/package-manager/proposal").exists());
    }
    assert!(!log.exists(), "offline commands attempted Git transport");
}

#[cfg(not(unix))]
#[test]
#[ignore = "offline command transport counter requires a Unix shell; offline local CLI tests run on every host"]
fn offline_unrecorded_git_fails_through_every_supported_command_without_transport() {}
