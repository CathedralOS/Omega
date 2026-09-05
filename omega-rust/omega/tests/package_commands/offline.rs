use super::fixture::{Fixture, assert_status};

#[test]
fn offline_local_install_update_check_and_inspection_use_the_normal_workflow() {
    let fixture = Fixture::new();
    let before = fixture.accepted_files();
    assert_status(
        &fixture.omega(&["install", "../dependency", "--offline"]),
        0,
    );
    fixture.assert_published(&before);
    fixture.write(
        "root/main.omg",
        "use arithmetic_kernels::main;\nmachine main() -> u64 { value() }\n",
    );
    let accepted = fixture.accepted_files();
    assert_status(&fixture.omega(&["--check", "--offline", "main.omg"]), 0);
    assert_status(&fixture.omega(&["audit", "packages", "--offline"]), 0);
    assert_eq!(fixture.accepted_files(), accepted);
    fixture.write("dependency/main.omg", "pub machine value() -> u64 { 41 }\n");
    assert_status(&fixture.omega(&["update", "--offline"]), 0);
    assert_ne!(fixture.accepted_files().1, accepted.1);
    assert_status(&fixture.omega(&["--offline", "--check", "main.omg"]), 0);
}

#[test]
fn offline_review_still_requires_decisions_and_resume_rechecks_source() {
    let fixture = Fixture::with_assumption();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency", "--offline"]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    assert_status(&fixture.omega(&["install", "--resume", "--offline"]), 3);
    let paths = fixture.review_paths(&output);
    for path in paths {
        let document = std::fs::read_to_string(&path).unwrap();
        let accepted = document
            .lines()
            .map(|line| {
                if line.starts_with("decision ") {
                    format!("{} accept\n", line.strip_suffix(" pending").unwrap())
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        assert_ne!(accepted, document);
        std::fs::write(path, accepted).unwrap();
    }
    assert_status(&fixture.omega(&["install", "--resume", "--offline"]), 0);
    fixture.assert_published(&before);
    fixture.write(
        "dependency/main.omg",
        "machine invalid() -> u64 ensures result == 0 { 1 }\n",
    );
    let accepted = fixture.accepted_files();
    assert_status(&fixture.omega(&["update", "--offline"]), 1);
    assert_eq!(fixture.accepted_files(), accepted);
}

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
    fixture.write("root/build.omg", &format!("machine build(builder: &mut Build) {{ builder.package(\"cli-project\"); builder.depend(Source::Git {{ repository: \"{locator}\", revision: \"HEAD\" }}); }}\n"));
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
