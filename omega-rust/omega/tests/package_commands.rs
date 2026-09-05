//! Package commands exercise the shipped binary and real local source acquisition.

#[path = "package_commands/authority.rs"]
mod authority;
#[path = "package_commands/diagnostics.rs"]
mod diagnostics;
#[path = "package_commands/fixture.rs"]
mod fixture;
#[path = "package_commands/generated.rs"]
mod generated;
#[path = "package_commands/inspection.rs"]
mod inspection;
#[path = "package_commands/remote.rs"]
mod remote;
#[path = "package_commands/remote_authority.rs"]
mod remote_authority;
#[path = "package_commands/remote_review.rs"]
mod remote_review;
#[path = "package_commands/workspace.rs"]
mod workspace;

use fixture::{Fixture, assert_status};

#[test]
fn local_install_publishes_build_and_lock_and_default_alias_imports() {
    let fixture = Fixture::new();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency"]);
    assert_status(&output, 0);
    fixture.assert_published(&before);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Published build.omg and omega.lock"));

    fixture.write(
        "root/main.omg",
        "use arithmetic_kernels::main;\nmachine main() -> u64 { value() }\n",
    );
    let accepted = fixture.accepted_files();
    assert_status(&fixture.omega(&["--check", "main.omg"]), 0);
    assert_eq!(
        fixture.accepted_files(),
        accepted,
        "checking must not rewrite the accepted pair"
    );
}

#[test]
fn explicit_alias_imports_and_project_and_multiple_targets_reach_manager() {
    let fixture = Fixture::new();
    let before = fixture.accepted_files();
    assert_status(
        &fixture.omega(&[
            "install",
            "../dependency",
            "--as",
            "numbers",
            "--project",
            ".",
            "--target",
            "linux_x64",
            "--target",
            "macos_arm64",
        ]),
        0,
    );
    fixture.assert_published(&before);
    let lock = fixture.lock();
    assert_eq!(lock.targets().len(), 2);
    assert!(lock.target(target::TargetProfile::LinuxX64).is_some());
    assert!(lock.target(target::TargetProfile::MacosArm64).is_some());
    let build = fixture.read("root/build.omg");
    assert!(build.contains("depend_as(\"numbers\""), "{build}");

    fixture.write(
        "root/main.omg",
        "use numbers::main;\nmachine main() -> u64 { value() }\n",
    );
    assert_status(
        &fixture.omega(&["--check", "--target", "linux_x86_64", "main.omg"]),
        0,
    );
}

#[test]
fn unchanged_updates_publish_for_all_packages_declared_name_and_alias() {
    let fixture = Fixture::new();
    assert_status(
        &fixture.omega(&["install", "../dependency", "--as", "numbers"]),
        0,
    );
    let before = fixture.accepted_files();
    for arguments in [
        &["update"][..],
        &["update", "arithmetic-kernels"],
        &["update", "numbers"],
    ] {
        let output = fixture.omega(arguments);
        assert_status(&output, 0);
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("Published build.omg and omega.lock")
        );
        assert_eq!(
            fixture.accepted_files().0,
            before.0,
            "unchanged update altered dependency declarations"
        );
        fixture.lock();
    }
}

#[test]
fn missing_source_preserves_an_existing_accepted_pair() {
    let fixture = Fixture::new();
    assert_status(&fixture.omega(&["install", "../dependency"]), 0);
    let before = fixture.accepted_files();
    assert_status(&fixture.omega(&["install", "../missing-source"]), 1);
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn invalid_dependency_proof_leaves_build_and_absent_lock_unchanged() {
    let fixture = Fixture::new();
    fixture.write(
        "dependency/main.omg",
        include_str!("../../../tests/omega/fail/domains/exit_ensures_unproven/main.omg"),
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency"]);
    assert_status(&output, 1);
    let expected = "checked compilation failed for package `arithmetic-kernels`";
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn project_without_build_rejects_without_creating_accepted_files() {
    let fixture = Fixture::new();
    std::fs::remove_file(fixture.path("root/build.omg")).unwrap();
    let before = fixture.accepted_files();
    assert_status(&fixture.omega(&["install", "../dependency"]), 1);
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn help_and_invalid_flags_have_cli_exit_statuses_and_no_accepted_writes() {
    let fixture = Fixture::new();
    let before = fixture.accepted_files();
    for kind in ["install", "update"] {
        let output = fixture.omega(&[kind, "--help"]);
        assert_status(&output, 0);
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("usage: omega {kind}")));
        assert!(
            !fixture.path("root/build/package-manager").exists(),
            "help invoked the manager"
        );
    }
    for arguments in [
        vec!["install"],
        vec!["install", "../dependency", "--bogus"],
        vec!["update", "--as", "name"],
        vec!["update", "--rev", "v1"],
        vec!["install", "../dependency", "--to", "v1"],
        vec!["update", "--target", "not-a-target"],
        vec!["install", "--resume", "../dependency"],
        vec!["update", "--discard-review", "--target", "linux_x86_64"],
        vec!["update", "--resume", "--discard-review"],
        vec![
            "install",
            "../dependency",
            "--project",
            ".",
            "--project",
            ".",
        ],
        vec![
            "update",
            "--target",
            "linux_x64",
            "--target",
            "linux_x86_64",
        ],
    ] {
        let output = fixture.omega(&arguments);
        assert_status(&output, 2);
        assert!(String::from_utf8_lossy(&output.stderr).contains("usage: omega"));
        assert!(output.stdout.is_empty());
        assert!(
            !fixture.path("root/build/package-manager").exists(),
            "invalid arguments invoked the manager: {arguments:?}"
        );
        assert_eq!(fixture.accepted_files(), before);
    }
}

#[test]
fn initial_assumption_review_resumes_with_exact_document_decisions() {
    let fixture = Fixture::with_assumption();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let paths = fixture.review_paths(&output);
    assert_eq!(paths.len(), 1);
    assert_status(&fixture.omega(&["install", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    for path in paths {
        let document = std::fs::read_to_string(&path).unwrap();
        assert!(document.starts_with("omega-package-review 1\n"));
        let mut decisions = 0;
        let accepted = document
            .lines()
            .map(|line| {
                if line.starts_with("decision ")
                    && let Some(prefix) = line.strip_suffix(" pending")
                {
                    decisions += 1;
                    format!("{prefix} accept\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        assert!(
            decisions > 0,
            "blocking review had no pending decisions: {document}"
        );
        std::fs::write(path, accepted).unwrap();
    }
    assert_status(
        &fixture.omega(&["install", "--resume", "--project", "."]),
        0,
    );
    fixture.assert_published(&before);
}

#[test]
fn discard_review_preserves_accepted_files_and_prevents_resume() {
    let fixture = Fixture::with_assumption();
    let before = fixture.accepted_files();
    assert_status(&fixture.omega(&["install", "../dependency"]), 3);
    assert_eq!(fixture.accepted_files(), before);
    assert_status(&fixture.omega(&["install", "--discard-review"]), 0);
    assert_eq!(fixture.accepted_files(), before);
    assert_status(&fixture.omega(&["install", "--resume"]), 1);
    assert_eq!(fixture.accepted_files(), before);
}
