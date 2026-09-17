//! The ordinary CLI review proposes the `Console::exit_process` terminal
//! permission as its own decision row for an application that exits through
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
const EXIT_REQUIREMENT: &str = "path(Console::exit_process)";

fn console_exit_fixture() -> Fixture {
    let fixture = Fixture::new();
    // The fixture's relative std location resolves from its own directory;
    // the copied project names the repository checkout directly.
    let standard_library =
        fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../source/library/std"))
            .expect("bundled standard library checkout");
    let build = APP_BUILD.replace(
        "\"../../../source/library/std\"",
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

/// The exact pending decision line of the one added terminal-permission row.
fn permission_decision(section: &str) -> &str {
    let rows = section
        .split("\nchange ")
        .filter(|row| row.starts_with("terminal_permission added\n"))
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        panic!("expected one added terminal permission row: {section}");
    };
    assert!(row.contains(EXIT_REQUIREMENT), "{row}");
    assert!(row.contains("tag process_termination"), "{row}");
    assert!(row.contains("string \"Console\""), "{row}");
    row.lines()
        .find(|line| line.starts_with("decision ") && line.ends_with(" pending"))
        .unwrap_or_else(|| panic!("permission row has no exact pending decision: {row}"))
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

    // The application, not its std dependency, owns the permission decision.
    let application = package_section(&document, "console-exit-app");
    let decision = permission_decision(application);
    assert_eq!(
        application
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count(),
        1,
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

    // The row is a proposal: every other acceptance cannot publish without it.
    let accepted = accept_all(&document);
    let accepted_decision = decision.replace(" pending", " accept");
    fs::write(path, accepted.replace(&accepted_decision, decision)).unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 3);
    assert_eq!(fixture.accepted_files(), before);

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
        .collect::<Vec<_>>();
    let [permission_row] = permission_rows.as_slice() else {
        panic!("the lock must retain exactly one terminal permission row");
    };
    assert!(permission_row.canonical_text().contains(EXIT_REQUIREMENT));
    assert!(
        permission_row
            .canonical_text()
            .contains("tag process_termination")
    );
    assert_eq!(
        target.decisions().decisions().len(),
        document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .count()
    );

    // Native production now passes the package permission axis: the accepted
    // row rejoins the retained proposal, and the realization stops only at the
    // independently supplied receiving policy, which the CLI does not carry
    // yet (TWO-AXIS-TERMINAL-AUTHORITY-REVIEW). Before this row existed the
    // reviewer rejected the reachable exit leaf outright. Once the CLI supplies
    // a receiving policy this command exits 0 and the program exits 70.
    let output = fixture.omega(&["--accept-admissions", "--target", "macos_arm64", "main.omg"]);
    assert_status(&output, 1);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("receiving terminal-authority policy omits the accepted permission for"),
        "{stderr}"
    );
    assert!(stderr.contains(EXIT_REQUIREMENT), "{stderr}");
    assert!(
        !stderr.contains("receiving terminal-authority permission policy has no exact row"),
        "{stderr}"
    );
}
