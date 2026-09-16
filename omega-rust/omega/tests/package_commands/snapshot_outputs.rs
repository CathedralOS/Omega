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

fn snapshot_fixture(build: &str) -> Fixture {
    let fixture = Fixture::new();
    fixture.write("root/build.omg", build);
    fixture.write("root/main.omg", MAIN);
    fs::create_dir(fixture.path("root/templates")).unwrap();
    fixture.write("root/templates/banner.tmpl", TEMPLATE);
    fixture
}

fn audit(fixture: &Fixture) -> std::process::Output {
    fixture.omega(&["audit", "packages", "--target", "linux_x86_64", "--offline"])
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
