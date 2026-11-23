// Test fixture root for `BuildStagedOutputTree` file selection tests.

#[path = "file_selection_tests.rs"]
mod file_selection;

use super::BuildStagedOutputTree;
use crate::tree_from_entries::{OutputTreeEntry, from_entries};

fn baseline_tree() -> BuildStagedOutputTree {
    from_entries(&[
        OutputTreeEntry::directory(b"docs"),
        OutputTreeEntry::directory(b"docs/inner"),
        OutputTreeEntry::regular_file(b"docs/inner/a.txt", b"alpha", false),
        OutputTreeEntry::regular_file(b"docs/inner/gen.omg", b"data Generated {}\n", false),
        OutputTreeEntry::regular_file(b"report.txt", b"report-body", false),
        OutputTreeEntry::regular_file(b"z-last.bin", b"z", false),
    ])
    .expect("baseline retained tree")
}

#[test]
fn a_legacy_export_can_contain_binary_bytes_but_cannot_supply_them_as_source() {
    let path = b"older.omg";
    let tree = from_entries(&[OutputTreeEntry::regular_file(path, &[0xff], false)]).unwrap();
    let refusal = super::recover_included_output_sources(&tree, &[path.to_vec()]).unwrap_err();
    assert!(refusal[0].to_string().contains("must contain UTF-8 bytes"));
}
