//! The package-manager compile route runs every package build against a
//! captured snapshot of the package's sealed source custody and settles the
//! outputs the build declared through `builder.output.require`. The same
//! admission the compiler tests reach by hand-constructing a
//! `BuildSnapshotRequest` must be reached by an ordinary project through
//! `omega audit packages`. Later cases pin cross-occurrence custody: two
//! packages settling identically named inputs and outputs from their own
//! captured inventories, and one package's shared source preparation serving
//! independent occurrences under two requested targets.

use super::fixture::{Fixture, assert_status};
use std::fs;

const TARGET: target::TargetProfile = target::TargetProfile::LinuxX64;
const MAIN: &str = "machine main() {}\n";
const TEMPLATE: &str = "HELLO {{name}}\n";

/// Reads the template through the Source facet, then declares, writes, seals
/// and completes one required output.
const COMPLETING_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot_root");
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
    builder.package("snapshot_root");
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
    builder.package("snapshot_root");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    builder.output.fail(required, "generator unavailable");
}
"#;

/// Completes one obligation and fails another: no successful product set.
const MIXED_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot_root");
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
    builder.package("snapshot_root");
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
    builder.package("snapshot_root");
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
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert_eq!(fixture.accepted_files(), before);
    // The report surfaces the binding: captured inventory extent, each
    // settled required output, and the sealed staged entries under --details.
    let captured = format!(
        "build-snapshot captured-entries 5 captured-file-bytes {}",
        COMPLETING_BUILD.len() + MAIN.len() + TEMPLATE.len()
    );
    assert!(report.contains(&captured), "{report}");
    assert!(report.contains("settled-outputs 1\n"), "{report}");
    assert!(
        report.contains("  settled-output \"artifact.txt\"\n"),
        "{report}"
    );
    assert!(
        report.contains("sealed-outputs 1; each sealed entry: --details\n"),
        "{report}"
    );
    assert!(!report.contains("  sealed-output "), "{report}");
    let detailed = fixture.omega_with_env(
        &[
            "audit",
            "packages",
            "--target",
            "linux_x86_64",
            "--offline",
            "--details",
        ],
        &[("TMPDIR", fixture.path("scratch").to_str().unwrap())],
    );
    assert_status(&detailed, 0);
    let detailed = String::from_utf8_lossy(&detailed.stdout).into_owned();
    assert!(detailed.contains(&captured), "{detailed}");
    assert!(detailed.contains("sealed-outputs 1\n"), "{detailed}");
    assert!(
        detailed.contains("  sealed-output \"artifact.txt\" file 12 bytes\n"),
        "{detailed}"
    );

    let fresh = fixture.fresh_reviews(TARGET);
    let review = fresh
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "snapshot_root")
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
        .find(|review| review.key().name().as_str() == "snapshot_root")
        .expect("the root package is reviewed");
    let observation = review
        .build_observation_summary()
        .expect("an executed package build retains its observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "the retried obligation settles once");
    assert_eq!(settlements[0].relative_path(), b"retry.txt");
}

/// The two packages' identically named inputs and outputs differ in extent
/// (12 vs 18 bytes): a shared inventory or staged tree could not carry both.
const ROOT_TEMPLATE: &str = "ROOT BANNER\n";
const DEPENDENCY_TEMPLATE: &str = "DEPENDENCY BANNER\n";

/// The root's build declares the product dependency, then performs the same
/// `templates/banner.tmpl` read and `artifact.txt` completion as the
/// dependency's build — disambiguated only by package custody.
const NAMES_ROOT_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot_root");
    builder.depend(Source::Path { location: "../dependency" });
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 12];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 12);
    let source_close: i32 = builder.source.close(template_descriptor);

    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, &banner_bytes);
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#;

/// The dependency's build runs the same member names against its own
/// captured inventory; its template is longer so each package's sealed
/// `artifact.txt` carries provably distinct bytes.
const NAMES_DEPENDENCY_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot_dependency");
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 18];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 18);
    let source_close: i32 = builder.source.close(template_descriptor);

    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, &banner_bytes);
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#;

/// The root declares the dependency and requires `artifact.txt` but never
/// completes it; the dependency's occurrence completes the same name.
const OMITTING_ROOT_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot_root");
    builder.depend(Source::Path { location: "../dependency" });
    let required: RequiredOutput = builder.output.require("artifact.txt");
}
"#;

/// Completes `artifact.txt` only under the Linux target: the macOS child
/// leaves its identical obligation uncommitted.
const TARGET_DIVERGENT_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.package("snapshot_root");
    let required: RequiredOutput = builder.output.require("artifact.txt");
    transition builder.target {
        TargetProfile::LinuxX86_64 -> complete(builder, required)
        _ -> withheld(builder)
    }

    state complete(builder: &mut Build, required: RequiredOutput) {
        let required_path: &[u8] = required.path();
        let artifact: BuildPath = builder.output.resolve(required_path);
        let descriptor: i32 = builder.output.create(artifact, 438);
        let written: i64 = builder.output.write(descriptor, "linux artifact\n");
        let closed: i32 = builder.output.close(descriptor);
        let completion: OutputCompletion = builder.output.complete(required, artifact);
    }

    state withheld(builder: &mut Build) { }
}
"#;

fn shared_names_fixture(root_build: &str, dependency_build: &str) -> Fixture {
    let fixture = Fixture::new();
    fixture.write("root/build.omg", root_build);
    fixture.write("root/main.omg", MAIN);
    fs::create_dir(fixture.path("root/templates")).unwrap();
    fixture.write("root/templates/banner.tmpl", ROOT_TEMPLATE);
    fixture.write("dependency/build.omg", dependency_build);
    fixture.write("dependency/main.omg", MAIN);
    fs::create_dir(fixture.path("dependency/templates")).unwrap();
    fixture.write("dependency/templates/banner.tmpl", DEPENDENCY_TEMPLATE);
    fs::create_dir(fixture.path("scratch")).unwrap();
    fixture
}

#[test]
fn two_packages_settle_identically_named_inputs_and_outputs_from_their_own_snapshots() {
    let fixture = shared_names_fixture(NAMES_ROOT_BUILD, NAMES_DEPENDENCY_BUILD);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 0);
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(report.contains("fresh-analysis complete"), "{report}");
    // Each package section carries its own captured extent: identical member
    // names, different inventories.
    for (package, build, template) in [
        ("snapshot-root", NAMES_ROOT_BUILD, ROOT_TEMPLATE),
        (
            "snapshot-dependency",
            NAMES_DEPENDENCY_BUILD,
            DEPENDENCY_TEMPLATE,
        ),
    ] {
        let extent = format!(
            "build-snapshot captured-entries 5 captured-file-bytes {}",
            build.len() + MAIN.len() + template.len()
        );
        assert!(
            report.contains(&format!("fresh package \"{package}\"")),
            "{report}"
        );
        assert!(report.contains(&extent), "{package}: {report}");
    }
    assert_eq!(
        report
            .matches("  settled-output \"artifact.txt\"\n")
            .count(),
        2,
        "each occurrence settles its own artifact.txt: {report}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/artifact.txt").exists());
    assert!(!fixture.path("dependency/artifact.txt").exists());
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());

    // Each occurrence's sealed custody retains exactly the bytes its own
    // captured template supplied: one receipt or staged tree serving both
    // packages could not produce each package's distinct content.
    let fresh = fixture.fresh_reviews(TARGET);
    for (package, template) in [
        ("snapshot-root", ROOT_TEMPLATE),
        ("snapshot-dependency", DEPENDENCY_TEMPLATE),
    ] {
        let review = fresh
            .reviews()
            .iter()
            .find(|review| review.key().name().as_str() == package)
            .expect("both packages are reviewed");
        let observation = review
            .build_observation_summary()
            .expect("each executed build retains its observation");
        let inventory = observation
            .captured_source_inventory()
            .expect("each occurrence bound its own captured inventory");
        assert_eq!(inventory.entry_count(), 5, "{package}");
        let settlements = observation.required_output_settlements();
        assert_eq!(settlements.len(), 1, "{package}");
        assert_eq!(settlements[0].relative_path(), b"artifact.txt");
        let staged = observation
            .staged_output_tree()
            .expect("each occurrence retains its own staged custody");
        let entry = staged
            .sealed_entry(b"artifact.txt")
            .expect("each occurrence sealed its own artifact.txt");
        // `build-evaluation` is not a direct dependency of this crate, so
        // the sealed file bytes are compared through the entry's derived
        // Debug spelling.
        assert_eq!(
            format!("{:?}", entry.kind()),
            format!(
                "File {{ bytes: {:?}, executable: false }}",
                template.as_bytes()
            ),
            "{package}"
        );
    }
}

#[test]
fn one_package_settles_independent_occurrences_under_each_requested_target() {
    let fixture = snapshot_fixture(COMPLETING_BUILD);
    let before = fixture.accepted_files();
    let output = fixture.omega_with_env(
        &[
            "audit",
            "packages",
            "--target",
            "linux_x86_64",
            "--target",
            "macos_arm64",
            "--offline",
        ],
        &[("TMPDIR", fixture.path("scratch").to_str().unwrap())],
    );
    assert_status(&output, 0);
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    // Each target child ran its own occurrence with its own captured
    // snapshot and staged custody. Source preparation is retained across the
    // requested targets, so the second target's build reuses the shared
    // parsed-storage slot; that cache entry must carry no settlement or
    // output custody between occurrences.
    assert_eq!(
        report.matches("target omega.target-profile.v1:").count(),
        2,
        "{report}"
    );
    assert_eq!(
        report.matches("fresh-analysis complete").count(),
        2,
        "{report}"
    );
    assert!(report.contains("linux_x86_64"), "{report}");
    assert!(report.contains("macos_arm64"), "{report}");
    assert_eq!(
        report
            .matches("  settled-output \"artifact.txt\"\n")
            .count(),
        2,
        "each target's occurrence settles its own artifact.txt: {report}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/artifact.txt").exists());
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());
    for target in [
        target::TargetProfile::LinuxX64,
        target::TargetProfile::MacosArm64,
    ] {
        let fresh = fixture.fresh_reviews(target);
        let review = fresh
            .reviews()
            .iter()
            .find(|review| review.key().name().as_str() == "snapshot_root")
            .expect("the root package is reviewed for each target");
        let observation = review
            .build_observation_summary()
            .expect("each target's occurrence retains its observation");
        let settlements = observation.required_output_settlements();
        assert_eq!(settlements.len(), 1, "{}", target.target_name());
        assert_eq!(settlements[0].relative_path(), b"artifact.txt");
    }
}

#[test]
fn one_targets_uncommitted_set_does_not_hide_another_targets_completion() {
    let fixture = snapshot_fixture(TARGET_DIVERGENT_BUILD);
    let before = fixture.accepted_files();
    let output = fixture.omega_with_env(
        &[
            "audit",
            "packages",
            "--target",
            "linux_x86_64",
            "--target",
            "macos_arm64",
            "--offline",
        ],
        &[("TMPDIR", fixture.path("scratch").to_str().unwrap())],
    );
    // The Linux child completed its obligation; the macOS child left the
    // same obligation uncommitted. Per-target children retain independent
    // outcomes: the macOS section reports its own rejection while the Linux
    // section still carries its completed settlement, and the command exits
    // nonzero rather than publishing a partial acceptance.
    assert_status(&output, 1);
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    assert_eq!(
        report.matches("target omega.target-profile.v1:").count(),
        2,
        "{report}"
    );
    assert_eq!(
        report.matches("fresh-analysis complete").count(),
        1,
        "{report}"
    );
    assert_eq!(
        report.matches("fresh-analysis unavailable").count(),
        1,
        "{report}"
    );
    assert!(
        report.contains("  settled-output \"artifact.txt\"\n"),
        "{report}"
    );
    let combined = combined_output(&output);
    assert!(
        combined
            .contains("required output `artifact.txt` of `build` was declared but never completed"),
        "{combined}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/artifact.txt").exists());
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());
}

/// An `artifact_only` application declares no executable product: its build
/// still runs against the captured snapshot and publishes only the sealed
/// required-artifact set. This is the package-route witness for the
/// artifact-only half of the acceptance pair; the compiler suite covers the
/// same admission through a hand-constructed `BuildSnapshotRequest`.
const ARTIFACT_ONLY_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("snapshot_artifact_app");
    builder.artifact_only();
    let template: BuildPath = builder.source.resolve("templates/banner.tmpl");
    let template_descriptor: i32 = builder.source.open(template, 0);
    let mut banner_bytes: [u8; 7];
    let read_count: i64 = builder.source.read(template_descriptor, &mut banner_bytes, 7);
    let source_close: i32 = builder.source.close(template_descriptor);

    let required: RequiredOutput = builder.output.require("artifact.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, &banner_bytes);
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#;

/// The executable route: an ordinary application binds a program root and
/// still settles a companion required output through the same sealed staged
/// custody — the executable-with-companion half of the acceptance pair.
const EXECUTABLE_COMPANION_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("snapshot_companion_app");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    let required: RequiredOutput = builder.output.require("companion.txt");
    let required_path: &[u8] = required.path();
    let artifact: BuildPath = builder.output.resolve(required_path);
    let descriptor: i32 = builder.output.create(artifact, 438);
    let written: i64 = builder.output.write(descriptor, "companion\n");
    let closed: i32 = builder.output.close(descriptor);
    let completion: OutputCompletion = builder.output.complete(required, artifact);
}
"#;

/// The bound root's declared machine. `roots.bind` names `Main::main`, so the
/// executable-route fixture needs a real receiver machine to select.
const EXECUTABLE_MAIN: &str = "data Main {}\nmachine Main::main(&mut self) { }\n";

/// `artifact_only` admits no executable route: binding a program root must
/// reject even though the modifier is otherwise a valid declaration.
const ARTIFACT_ONLY_ROOTS_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("snapshot_artifact_roots");
    builder.artifact_only();
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#;

/// `artifact_only` without a completed required output publishes nothing:
/// the modifier exists to ship artifacts, so an empty artifact set rejects.
const ARTIFACT_ONLY_EMPTY_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("snapshot_artifact_empty");
    builder.artifact_only();
}
"#;

fn application_fixture(build: &str, main: &str) -> Fixture {
    let fixture = Fixture::new();
    fixture.write("root/build.omg", build);
    fixture.write("root/main.omg", main);
    fs::create_dir(fixture.path("root/templates")).unwrap();
    fixture.write("root/templates/banner.tmpl", TEMPLATE);
    fs::create_dir(fixture.path("scratch")).unwrap();
    fixture
}

#[test]
fn artifact_only_application_build_settles_its_required_output() {
    let fixture = application_fixture(ARTIFACT_ONLY_BUILD, MAIN);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 0);
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(report.contains("fresh-analysis complete"), "{report}");
    let captured = format!(
        "build-snapshot captured-entries 5 captured-file-bytes {}",
        ARTIFACT_ONLY_BUILD.len() + MAIN.len() + TEMPLATE.len()
    );
    assert!(report.contains(&captured), "{report}");
    assert!(report.contains("settled-outputs 1\n"), "{report}");
    assert!(
        report.contains("  settled-output \"artifact.txt\"\n"),
        "{report}"
    );
    assert!(
        report.contains("sealed-outputs 1; each sealed entry: --details\n"),
        "{report}"
    );
    assert_eq!(fixture.accepted_files(), before);
    // Artifact-only publishes no executable product and never writes into the
    // project: the settled artifact set stays in sealed staged custody.
    assert!(!fixture.path("root/artifact.txt").exists());
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    assert_eq!(
        snapshot_residue(&fixture),
        Vec::<String>::new(),
        "a settled artifact-only occurrence releases its private snapshot"
    );

    let fresh = fixture.fresh_reviews(TARGET);
    let review = fresh
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "snapshot_artifact_app")
        .expect("the root application is reviewed");
    let observation = review
        .build_observation_summary()
        .expect("the artifact-only build retains its observation");
    assert_eq!(
        observation
            .captured_source_inventory()
            .expect("the artifact-only occurrence bound a captured snapshot")
            .entry_count(),
        5,
        "root, build.omg, main.omg, the template directory, and the template"
    );
    let settlements = observation.required_output_settlements();
    assert_eq!(settlements.len(), 1, "exactly one obligation settled");
    assert_eq!(settlements[0].relative_path(), b"artifact.txt");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"artifact.txt")
        .expect("the completed artifact is discoverable in sealed custody");
    assert_eq!(
        format!("{:?}", entry.kind()),
        format!(
            "File {{ bytes: {:?}, executable: false }}",
            &TEMPLATE.as_bytes()[..7]
        ),
        "the sealed artifact carries exactly the bytes the captured template supplied"
    );
}

#[test]
fn executable_application_build_settles_a_companion_required_output() {
    let fixture = application_fixture(EXECUTABLE_COMPANION_BUILD, EXECUTABLE_MAIN);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 0);
    let report = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(report.contains("fresh-analysis complete"), "{report}");
    let captured = format!(
        "build-snapshot captured-entries 5 captured-file-bytes {}",
        EXECUTABLE_COMPANION_BUILD.len() + EXECUTABLE_MAIN.len() + TEMPLATE.len()
    );
    assert!(report.contains(&captured), "{report}");
    assert!(report.contains("settled-outputs 1\n"), "{report}");
    assert!(
        report.contains("  settled-output \"companion.txt\"\n"),
        "{report}"
    );
    assert!(
        report.contains("sealed-outputs 1; each sealed entry: --details\n"),
        "{report}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/companion.txt").exists());
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());

    let fresh = fixture.fresh_reviews(TARGET);
    let review = fresh
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "snapshot_companion_app")
        .expect("the root application is reviewed");
    let observation = review
        .build_observation_summary()
        .expect("the executable build retains its observation");
    let settlements = observation.required_output_settlements();
    assert_eq!(
        settlements.len(),
        1,
        "the companion obligation settles once"
    );
    assert_eq!(settlements[0].relative_path(), b"companion.txt");
    let staged = observation
        .staged_output_tree()
        .expect("completed outputs remain in staged custody");
    let entry = staged
        .sealed_entry(b"companion.txt")
        .expect("the completed companion is discoverable in sealed custody");
    assert_eq!(
        format!("{:?}", entry.kind()),
        "File { bytes: [99, 111, 109, 112, 97, 110, 105, 111, 110, 10], executable: false }",
        "the sealed companion carries the bytes the build wrote"
    );
}

#[test]
fn artifact_only_application_build_rejects_executable_root_bindings() {
    let fixture = application_fixture(ARTIFACT_ONLY_ROOTS_BUILD, EXECUTABLE_MAIN);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 1);
    let combined = combined_output(&output);
    assert!(
        combined.contains("may not bind executable roots or select boundary providers"),
        "{combined}"
    );
    assert!(!combined.contains("fresh-analysis complete"), "{combined}");
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    assert_eq!(
        snapshot_residue(&fixture),
        Vec::<String>::new(),
        "a rejected occurrence still releases its private snapshot"
    );
}

#[test]
fn artifact_only_application_build_without_a_completed_output_rejects() {
    let fixture = application_fixture(ARTIFACT_ONLY_EMPTY_BUILD, MAIN);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 1);
    let combined = combined_output(&output);
    assert!(
        combined.contains("completed no required output"),
        "{combined}"
    );
    assert!(!combined.contains("fresh-analysis complete"), "{combined}");
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/build/package-manager/proposal").exists());
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());
}

#[test]
fn a_settled_dependency_output_cannot_carry_the_roots_uncommitted_set() {
    // The dependency builds first and completes its `artifact.txt`; the
    // root's occurrence then leaves its own identical obligation
    // uncompleted. The audit must reject: one occurrence's settled output
    // never publishes through another occurrence's uncommitted set.
    let fixture = shared_names_fixture(OMITTING_ROOT_BUILD, NAMES_DEPENDENCY_BUILD);
    let before = fixture.accepted_files();
    let output = audit(&fixture);
    assert_status(&output, 1);
    let combined = combined_output(&output);
    assert!(
        combined
            .contains("required output `artifact.txt` of `build` was declared but never completed"),
        "{combined}"
    );
    assert!(!combined.contains("fresh-analysis complete"), "{combined}");
    assert_eq!(fixture.accepted_files(), before);
    assert!(!fixture.path("root/artifact.txt").exists());
    assert!(!fixture.path("dependency/artifact.txt").exists());
    assert_eq!(snapshot_residue(&fixture), Vec::<String>::new());
}
