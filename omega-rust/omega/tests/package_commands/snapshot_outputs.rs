//! The package-manager compile route runs every package build against a
//! captured snapshot of the package's sealed source custody and settles the
//! outputs the build declared through `builder.output.require`. The same
//! admission the compiler tests reach by hand-constructing a
//! `BuildSnapshotRequest` must be reached by an ordinary project through
//! `omega audit packages`.

use super::fixture::{Fixture, assert_status};
use std::fs;

const TARGET: target::TargetProfile = target::TargetProfile::LinuxX64;
const MAIN: &str = "machine main() {}\n";
const TEMPLATE: &str = "HELLO {{name}}\n";

/// Reads the template through the Source facet, then declares, writes, seals
/// and completes one required output.
const COMPLETING_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot-root");
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 7];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 7);
    let source_close: i32 = builder.source.close(template_descriptor);

    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "banner read\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#;

/// Declares the same required output after the same template read but never
/// completes it.
const OMITTING_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot-root");
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 7];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 7);
    let source_close: i32 = builder.source.close(template_descriptor);

    let required: RequiredOutput = builder.output.require("artifact.txt");
}
"#;

/// Settles its obligation with `fail`: the failure is sticky and the
/// diagnostic reaches the customer.
const FAILING_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot-root");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    builder.output.fail(required, "generator unavailable");
}
"#;

/// Completes one obligation and fails another: no successful product set.
const MIXED_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot-root");
    let first: RequiredOutput = builder.output.require("first.txt");
    let second: RequiredOutput = builder.output.require("second.txt");
    let first_path: &[u8] = first.path();
    let artifact: BuildPath = builder.output.resolve(first_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "first\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(first, artifact);
    builder.output.fail(second, "second generator unavailable");
}
"#;

/// Presents an authored `RequiredOutput {}` as an obligation: evaluation
/// halts before settlement, so the occurrence never reaches the staged
/// custody path.
const FORGED_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot-root");
    let forged: RequiredOutput = RequiredOutput {};
    let artifact: BuildPath = builder.output.resolve("artifact.txt");
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "forged\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(forged, artifact);
}
"#;

/// Completes against an unclosed writer, receives its obligation and file
/// custody back through `OutputCompletion::Retry`, and completes once the
/// writer is closed.
const RETRYING_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot-root");
    let required: RequiredOutput = builder.output.require("retry.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "retried\n");
    let first: OutputCompletion = builder.output.complete(required, artifact);
    transition first {
        OutputCompletion::Retry { obligation, file } -> finish(builder, obligation, file, descriptor)
        _ -> unexpected(builder)
    }

    state finish(
        builder: &mut Build,
        obligation: RequiredOutput,
        file: BuildPath,
        descriptor: i32
    ) {
        let closed: i32 = builder.output.close(descriptor);
        let second: OutputCompletion = builder.output.complete(obligation, file);
    }

    state unexpected(builder: &mut Build) {
        builder.log.write_line("unexpected completion verdict");
    }
}
"#;

fn snapshot_fixture(build: &str) -> Fixture {
    let fixture = Fixture::new();
    fixture.write("root/build.omg", build);
    fixture.write("root/main.omg", MAIN);
    fs::create_dir(fixture.path("root/templates")).unwrap();
    fixture.write("root/templates/banner.tmpl", TEMPLATE);
    // The child's scratch directory is fixture-private so the test can see
    // exactly what an occurrence leaves behind.
    fs::create_dir(fixture.path("scratch")).unwrap();
    fixture
}

fn audit(fixture: &Fixture) -> std::process::Output {
    let scratch = fixture.path("scratch");
    fixture.omega_with_env(
        &["audit", "packages", "--target", "linux_x86_64", "--offline"],
        &[("TMPDIR", scratch.to_str().unwrap())],
    )
}

/// Captured-source snapshots the child materialized and never released.
fn snapshot_residue(fixture: &Fixture) -> Vec<String> {
    let mut residue = fs::read_dir(fixture.path("scratch"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("omega-captured-source-"))
        .collect::<Vec<_>>();
    residue.sort();
    residue
}

fn combined_output(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn package_build_reads_its_template_through_the_captured_snapshot_and_completes_a_required_output()
{
    let fixture = snapshot_fixture(COMPLETING_BUILD);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 0);
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert_eq!(fixture.accepted_files(), before);

    let fresh = fixture.fresh_reviews(TARGET);
    let review = fresh
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "snapshot-root")
        .expect("the root package is reviewed");
    let observation = review
        .build_observation_summary()
        .expect("an executed package build retains its observation");
    let inventory = observation
        .captured_source_inventory()
        .expect("the manager route binds the build to a captured source snapshot");
    assert_eq!(
        inventory.entry_count(),
        5,
        "root, build.omg, main.omg, the template directory, and the template"
    );
    assert_eq!(
        inventory.file_bytes(),
        (COMPLETING_BUILD.len() + MAIN.len() + TEMPLATE.len()) as u64
    );
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "exactly one declared output settles");
    assert_eq!(settlements[0].relative_path(), b"artifact.txt");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    assert!(
        staged.sealed_entry(b"artifact.txt").is_some(),
        "the completed required output is discoverable in sealed custody"
    );
    assert!(!fixture.path("root/artifact.txt").exists());
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    assert_eq!(
        snapshot_residue(&fixture),
        Vec::<String>::new(),
        "a settled occurrence releases its private snapshot"
    );
}

#[test]
fn package_build_that_never_completes_a_declared_output_rejects_review() {
    let fixture = snapshot_fixture(OMITTING_BUILD);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 1);
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined
            .contains("required output `artifact.txt` of `build` was declared but never completed"),
        "{combined}"
    );
    assert!(!combined.contains("fresh-analysis complete"), "{combined}");
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/artifact.txt").exists());
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
}

#[test]
fn explicit_failure_and_partial_completion_reject_and_publish_nothing() {
    for (build, expected) in [
        (
            FAILING_BUILD,
            "required output `artifact.txt` of `build` failed: generator unavailable",
        ),
        (
            MIXED_BUILD,
            "required output `second.txt` of `build` failed: second generator unavailable",
        ),
    ] {
        let fixture = snapshot_fixture(build);
        let before = fixture.accepted_files();
        let output = audit(&fixture);
        assert_status(&output, 1);
        let combined = combined_output(&output);
        assert!(combined.contains(expected), "{combined}");
        assert!(!combined.contains("fresh-analysis complete"), "{combined}");
        assert_eq!(fixture.accepted_files(), before);
        assert!(!fixture.path("root/first.txt").exists());
        assert!(!fixture.path("root/artifact.txt").exists());
        assert!(!fixture.path("root/build/package-manager/proposal").exists());
        assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());
    }
}

#[test]
fn halted_build_releases_its_captured_snapshot_and_rejects_the_forged_obligation() {
    let fixture = snapshot_fixture(FORGED_BUILD);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 1);
    let combined = combined_output(&output);
    assert!(
        combined.contains("not a compiler-issued required output obligation"),
        "{combined}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/artifact.txt").exists());
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    // Evaluation halted before settlement ever ran; the occurrence's private
    // snapshot must still be released rather than left as read-only residue.
    assert_eq!(
        snapshot_residue(&fixture),
        Vec::<String>::new(),
        "a halted occurrence releases its private snapshot"
    );
}

#[test]
fn completion_error_returns_custody_and_an_explicit_retry_completes() {
    let fixture = snapshot_fixture(RETRYING_BUILD);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 0);
    let combined = combined_output(&output);
    assert!(combined.contains("fresh-analysis complete"), "{combined}");
    assert!(
        !combined.contains("unexpected completion verdict"),
        "{combined}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());

    let fresh = fixture.fresh_reviews(TARGET);
    let review = fresh
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "snapshot-root")
        .expect("the root package is reviewed");
    let observation = review
        .build_observation_summary()
        .expect("an executed package build retains its observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "the retried obligation settles once");
    assert_eq!(settlements[0].relative_path(), b"retry.txt");
}
