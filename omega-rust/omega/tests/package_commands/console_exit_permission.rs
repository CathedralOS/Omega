//! The ordinary CLI review proposes the `Console::exit_process`,
//! `Console::write_byte` and `Console::read_byte` terminal permissions as
//! their own decision rows for an application that writes and exits through
//! the bundled standard library reached as a path dependency.
//!
//! Every invocation here checks `source/library/std`, so the test is slow by
//! nature; it drives the shipped binary exactly as a downstream runner would.

use super::fixture::{Fixture, assert_status};
use std::fs;
use std::path::Path;

const TARGET: target::TargetProfile = target::TargetProfile::MacosArm64;
const APP_BUILD: &str =
    include_str!("../../../../tests/fixtures/packages/console-exit-app/build.omg");
const APP_SOURCE: &str =
    include_str!("../../../../tests/fixtures/packages/console-exit-app/main.omg");
const STANDARD_LIBRARY_LOCATION: &str = "../../../../source/library/std";
const EXIT_REQUIREMENT: &str = "path(Console::exit_process)";
/// Every compiler-intrinsic leaf of the selected std Console provider, with
/// the permission tag the review proposes for it.
const PERMISSIONS: [(&str, &str); 3] = [
    (EXIT_REQUIREMENT, "tag process_termination"),
    ("path(Console::write_byte)", "tag process_output"),
    ("path(Console::read_byte)", "tag process_input"),
];

fn console_exit_fixture() -> Fixture {
    let fixture = Fixture::new();
    // The fixture's relative std location resolves from its own directory;
    // the copied project names the repository checkout directly.
    let standard_library =
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library/std"))
            .expect("bundled standard library checkout");
    let checked_in_project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/packages/console-exit-app");
    assert_eq!(
        fs::canonicalize(checked_in_project.join(STANDARD_LIBRARY_LOCATION))
            .expect("the checked-in fixture's dependency resolves before it is copied"),
        standard_library,
    );
    let build = APP_BUILD.replace(
        &format!("{STANDARD_LIBRARY_LOCATION:?}"),
        &format!("{:?}", standard_library.to_str().unwrap()),
    );
    assert_ne!(build, APP_BUILD);
    fixture.write("root/build.omg", &build);
    fixture.write("root/main.omg", APP_SOURCE);
    fixture
}

fn package_section<'a>(document: &'a str, package: &str) -> &'a str {
    document
        .split("\npackage ")
        .find(|section| section.starts_with(&format!("\"{package}\" ")))
        .unwrap_or_else(|| panic!("missing package {package}: {document}"))
        .split("end-package")
        .next()
        .unwrap()
}

/// The exact pending decision line of each added terminal-permission row, in
/// the order of `PERMISSIONS`. Every row names the std Console schema and
/// exactly its own requirement and class.
fn permission_decisions(section: &str) -> Vec<&str> {
    let rows = section
        .split("\nchange ")
        .filter(|row| row.starts_with("terminal_permission added\n"))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        PERMISSIONS.len(),
        "one added terminal permission row per Console leaf: {section}"
    );
    PERMISSIONS
        .iter()
        .map(|(requirement, tag)| {
            let matching = rows
                .iter()
                .filter(|row| row.contains(requirement))
                .collect::<Vec<_>>();
            let [row] = matching.as_slice() else {
                panic!("expected one added row for {requirement}: {section}");
            };
            assert!(row.contains(tag), "{row}");
            assert!(row.contains("string \"Console\""), "{row}");
            for (_, other_tag) in PERMISSIONS.iter().filter(|(_, other)| other != tag) {
                assert!(!row.contains(other_tag), "{row}");
            }
            row.lines()
                .find(|line| line.starts_with("decision ") && line.ends_with(" pending"))
                .unwrap_or_else(|| panic!("permission row has no exact pending decision: {row}"))
        })
        .collect()
}

fn accept_all(document: &str) -> String {
    document
        .lines()
        .map(|line| {
            if line.starts_with("decision ") {
                format!("{} accept\n", line.strip_suffix(" pending").unwrap())
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}

#[test]
fn console_exit_permission_is_an_explicit_decision_that_the_lock_retains() {
    let fixture = console_exit_fixture();
    let before = fixture.accepted_files();
    let output = fixture.omega(&["update", "--target", "macos_arm64"]);
    assert_status(&output, 3);
    assert_eq!(fixture.accepted_files(), before);
    let paths = fixture.review_paths(&output);
    assert_eq!(paths.len(), 1, "one explicitly selected target");
    let path = &paths[0];
    let document = fs::read_to_string(path).unwrap();
    assert!(document.contains("baseline none\n"), "{document}");

    // The application, not its std dependency, owns the permission decisions:
    // one per compiler-intrinsic Console leaf and nothing else.
    let application = package_section(&document, "console-exit-app");
    let decisions = permission_decisions(application);
    assert_eq!(
        application
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count(),
        PERMISSIONS.len(),
        "{application}"
    );
    let standard_library = package_section(&document, "omega-language-std");
    assert!(
        !standard_library.contains("change terminal_permission"),
        "{standard_library}"
    );
    assert!(
        standard_library.contains("change dangerous_capability added\n"),
        "{standard_library}"
    );

    // Each row is a proposal: every other acceptance cannot publish without it.
    let accepted = accept_all(&document);
    for decision in &decisions {
        let accepted_decision = decision.replace(" pending", " accept");
        fs::write(path, accepted.replace(&accepted_decision, decision)).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
    }

    fs::write(path, &accepted).unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 0);
    assert_ne!(fixture.accepted_files(), before);
    let lock = fixture.lock();
    let target = lock.target(TARGET).expect("reviewed target");
    let permission_rows = target
        .baselines()
        .iter()
        .flat_map(|baseline| baseline.rows())
        .filter(|row| row.kind().as_str() == "terminal_permission")
        .map(|row| row.canonical_text())
        .collect::<Vec<_>>();
    assert_eq!(
        permission_rows.len(),
        PERMISSIONS.len(),
        "the lock must retain exactly one terminal permission row per leaf: {permission_rows:?}"
    );
    for (requirement, tag) in PERMISSIONS {
        assert!(
            permission_rows
                .iter()
                .any(|row| row.contains(requirement) && row.contains(tag)),
            "{requirement} {tag}: {permission_rows:?}"
        );
    }
    assert_eq!(
        target.decisions().decisions().len(),
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
    );

    // Package acceptance supplies no receiver-admission policy. Pin the native
    // outcome as well: merely excluding the retired permission diagnostics
    // would let any later compiler failure silently satisfy this witness.
    let accepted_files = fixture.accepted_files();
    let native_directory = fixture.path("root/build/native-production");
    assert!(!native_directory.exists());
    let output = fixture.omega(&[
        "--accept-admissions",
        "--target",
        "macos_arm64",
        "--build-dir",
        native_directory.to_str().unwrap(),
        "main.omg",
    ]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let mut publications = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("published native output to "));
        let published = Path::new(
            publications
                .next()
                .unwrap_or_else(|| panic!("missing native publication: {stdout}")),
        );
        assert!(publications.next().is_none(), "{stdout}");
        assert!(published.starts_with(&native_directory), "{stdout}");
        let metadata = fs::metadata(published).expect("reported native output exists");
        assert!(
            metadata.is_file() && metadata.len() > 0,
            "published native output is a nonempty file: {stdout}"
        );
    } else {
        // The current customer reaches physical legalization and stops at its
        // custody check. This exact blocker is the remaining native obligation;
        // no other rejection establishes successful package-to-native handoff.
        assert_status(&output, 1);
        assert_eq!(
            stderr.trim(),
            concat!(
                "cannot realize accepted package production: [Diagnostic { severity: Error, ",
                "message: \"native artifact identity physical pipeline failed: ",
                "common physical staging failed: Selection(Legalization(SourceCustodyMismatch))\", ",
                "source_span: None }]",
            ),
        );
        assert!(stdout.trim().is_empty(), "{stdout}");
    }
    assert_eq!(
        fixture.accepted_files(),
        accepted_files,
        "native compilation preserves the accepted project",
    );
}
