//! Fixtures shared by the range, storage and entry canaries.

#[path = "ranges_storage_and_entries/bitwise_calls_and_conversions.rs"]
mod bitwise_calls_and_conversions;
#[path = "ranges_storage_and_entries/entry_results_and_carrier_writes.rs"]
mod entry_results_and_carrier_writes;
#[path = "../fixture_rosters/ranges_storage_and_entries.rs"]
pub(super) mod fixture_roster;
#[path = "ranges_storage_and_entries/frame_and_nested_indexing.rs"]
mod frame_and_nested_indexing;
#[path = "ranges_storage_and_entries/guarded_ranges_and_indexed_fields.rs"]
mod guarded_ranges_and_indexed_fields;
#[path = "ranges_storage_and_entries/slices_subslices_and_carriers.rs"]
mod slices_subslices_and_carriers;
#[path = "ranges_storage_and_entries/versions_wire_and_const_lengths.rs"]
mod versions_wire_and_const_lengths;

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
