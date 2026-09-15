//! Fixtures shared by the artifact footprint canaries.

#[path = "../fixture_rosters/artifact_footprints.rs"]
pub(super) mod fixture_roster;
#[path = "artifact_footprints/pointee_and_indexed_copies.rs"]
mod pointee_and_indexed_copies;
#[path = "artifact_footprints/text_assembly_and_wire_reads.rs"]
mod text_assembly_and_wire_reads;

use crate::{Command, CompileReport};

fn assert_native_exit_code(
    report: &CompileReport,
    expected: i32,
    fixture: &str,
    expectation: &str,
) {
    let executable = report
        .checked_native_executable_path()
        .unwrap_or_else(|| panic!("{fixture} lost its exact executable publication receipt"));
    let output = Command::new(executable)
        .output()
        .unwrap_or_else(|error| panic!("{fixture} should run: {error}"));
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{expectation}; expected exit {expected}, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
