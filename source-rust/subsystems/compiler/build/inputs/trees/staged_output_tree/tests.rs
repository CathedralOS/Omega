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
fn an_explicit_output_tree_binds_its_symbolic_link_target() {
    use super::BuildStagedOutputEntryInput as Entry;
    let tree = from_entries(&[
        OutputTreeEntry::regular_file(b"file", b"value", false),
        OutputTreeEntry::symbolic_link(b"link", b"file"),
    ])
    .unwrap();
    let reopen = |target: &[u8]| {
        BuildStagedOutputTree::from_retained_parts(
            &tree.commitment(),
            [
                (
                    b"link".to_vec(),
                    Entry::Symlink {
                        target: target.to_vec(),
                    },
                ),
                (
                    b"file".to_vec(),
                    Entry::File {
                        bytes: b"value".as_slice().into(),
                        executable: false,
                    },
                ),
            ],
        )
    };
    assert_eq!(reopen(b"file").unwrap(), tree);
    assert!(reopen(b"other").is_err());
}
