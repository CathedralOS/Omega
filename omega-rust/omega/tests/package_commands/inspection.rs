use super::fixture::{Fixture, assert_status};
use std::fs;
use std::process::Output;

fn text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn declare_dependency(fixture: &Fixture) {
    fixture.write("root/build.omg", "machine build(builder: &mut Build) {\n    builder.package(\"cli-project\");\n    builder.depend(Source::Path { location: \"../dependency\" });\n}\n");
}

fn install(fixture: &Fixture) {
    assert_status(
        &fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]),
        0,
    );
}

#[test]
fn unlocked_inspection_checks_the_graph_without_creating_acceptance() {
    let fixture = Fixture::new();
    declare_dependency(&fixture);
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]);
    assert_status(&output, 0);
    let report = text(&output);
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(report.contains("accepted-policy none"), "{report}");
    assert!(report.contains("arithmetic-kernels"), "{report}");
    assert!(report.contains("arithmetic_kernels"), "{report}");
    assert!(report.contains("value"), "{report}");
    assert!(!report.lines().any(|line| line.starts_with("decision ")));
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

#[test]
fn initial_assumption_inspection_reports_review_without_a_proposal() {
    let fixture = Fixture::with_assumption();
    declare_dependency(&fixture);
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]);
    assert_status(&output, 3);
    let report = text(&output);
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(report.contains("trusted_zero"), "{report}");
    assert!(!report.lines().any(|line| line.starts_with("decision ")));
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

#[test]
fn inspection_exposes_filesystem_api_reach_and_its_transitive_package_path() {
    let fixture = super::authority::authority_fixture(
        include_str!("../../../../tests/fixtures/packages/file-journal/build.omg"),
        include_str!("../../../../tests/fixtures/packages/file-journal/main.omg"),
    );
    declare_dependency(&fixture);
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]);
    assert_status(&output, 3);
    let report = text(&output);
    for expected in [
        "file-journal",
        "host-services",
        "FilesystemHost",
        "append",
        "dependency-path",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn accepted_targets_are_inspected_without_repeating_equal_policy() {
    let fixture = Fixture::new();
    assert_status(
        &fixture.omega(&[
            "install",
            "../dependency",
            "--target",
            "linux_x86_64",
            "--target",
            "macos_arm64",
        ]),
        0,
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--project", "."]);
    assert_status(&output, 0);
    let report = text(&output);
    assert_eq!(
        report.matches("fresh-analysis complete").count(),
        2,
        "{report}"
    );
    assert!(report.contains("linux_x86_64"), "{report}");
    assert!(report.contains("macos_arm64"), "{report}");
    assert!(
        report.contains("accepted-policy equal-to-fresh"),
        "{report}"
    );
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn current_root_policy_changes_are_reported_without_updating_the_lock() {
    let fixture = Fixture::new();
    install(&fixture);
    fixture.write(
        "root/main.omg",
        "pub const CHANGED: u64 = 11;\nmachine main() {}\n",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages", "--details"]);
    assert_status(&output, 3);
    let report = text(&output);
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(report.contains("CHANGED"), "{report}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn default_summary_is_readable_and_details_keep_full_policy() {
    let fixture = Fixture::new();
    install(&fixture);
    let before = fixture.accepted_files();
    let summary = fixture.omega(&["audit", "packages"]);
    let details = fixture.omega(&["audit", "packages", "--details"]);
    assert_status(&summary, 0);
    assert_status(&details, 0);
    let summary = text(&summary);
    let details = text(&details);
    assert!(
        summary.lines().count() < 200,
        "pure package summary should remain scannable"
    );
    assert!(summary.len() < details.len() / 2);
    assert!(!summary.contains("omega_package_policy_text"));
    assert!(details.contains("omega_package_policy_text"));
    assert!(summary.contains("value"));
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn changed_or_missing_dependency_keeps_accepted_policy_without_fresh_findings() {
    let fixture = Fixture::new();
    install(&fixture);
    let before = fixture.accepted_files();
    fixture.write("dependency/main.omg", "pub machine value() -> u64 { 99 }\n");
    for missing in [false, true] {
        if missing {
            fs::rename(
                fixture.path("dependency"),
                fixture.path("missing-dependency"),
            )
            .unwrap();
        }
        let output = fixture.omega(&["audit", "packages"]);
        assert_status(&output, 1);
        let report = text(&output);
        assert!(report.contains("fresh-analysis unavailable"), "{report}");
        assert!(!report.contains("fresh-analysis complete"), "{report}");
        assert!(report.contains("arithmetic-kernels"), "{report}");
        assert!(report.contains("value"), "{report}");
        assert_eq!(fixture.accepted_files(), before);
    }
}

#[test]
fn invalid_proof_is_unavailable_not_empty_capabilities() {
    let fixture = Fixture::new();
    install(&fixture);
    fixture.write(
        "root/main.omg",
        "machine main() -> u64 ensures result == 0 { 1 }\n",
    );
    let before = fixture.accepted_files();
    let output = fixture.omega(&["audit", "packages"]);
    assert_status(&output, 1);
    let report = text(&output);
    assert!(report.contains("fresh-analysis unavailable"), "{report}");
    assert!(!report.contains("fresh-analysis complete"), "{report}");
    assert_eq!(fixture.accepted_files(), before);
}

#[test]
fn audit_preserves_pending_proposal_and_refuses_publication_recovery() {
    let fixture = Fixture::with_assumption();
    assert_status(
        &fixture.omega(&["install", "../dependency", "--target", "linux_x86_64"]),
        3,
    );
    let proposal = fixture.read("root/build/package-manager/proposal");
    let before = fixture.accepted_files();
    // The accepted build still has no dependency: audit does not inspect or
    // accept the separate proposed installation.
    assert_status(
        &fixture.omega(&["audit", "packages", "--target", "linux_x86_64"]),
        0,
    );
    assert_eq!(
        fixture.read("root/build/package-manager/proposal"),
        proposal
    );
    assert_eq!(fixture.accepted_files(), before);
    fixture.write(
        "root/build/package-manager/pending",
        "unprocessed commit intent\n",
    );
    let output = fixture.omega(&["audit", "packages"]);
    assert_status(&output, 1);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("recover the pending package publication")
    );
    assert_eq!(fixture.accepted_files(), before);
    assert_eq!(
        fixture.read("root/build/package-manager/pending"),
        "unprocessed commit intent\n"
    );
}

#[test]
fn unknown_accepted_target_and_malformed_lock_do_not_refresh_sources() {
    let fixture = Fixture::new();
    install(&fixture);
    let before = fixture.accepted_files();
    fs::rename(
        fixture.path("dependency"),
        fixture.path("missing-dependency"),
    )
    .unwrap();
    let output = fixture.omega(&["audit", "packages", "--target", "macos_arm64"]);
    assert_status(&output, 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("no accepted target"));
    assert_eq!(fixture.accepted_files(), before);
    fixture.write("root/omega.lock", "not a package lock\n");
    let malformed = fixture.accepted_files();
    assert_status(&fixture.omega(&["audit", "packages"]), 1);
    assert_eq!(fixture.accepted_files(), malformed);
}
