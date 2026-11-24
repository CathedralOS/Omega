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
    use super::{
        RetainedStagedOutputEntry, RetainedStagedOutputEntryKind, commitment_for_retained_entries,
    };
    let entries = vec![
        RetainedStagedOutputEntry {
            relative_path: b"file".to_vec(),
            kind: RetainedStagedOutputEntryKind::File {
                bytes: b"value".as_slice().into(),
                executable: false,
            },
        },
        RetainedStagedOutputEntry {
            relative_path: b"link".to_vec(),
            kind: RetainedStagedOutputEntryKind::Symlink {
                target: b"file".to_vec(),
            },
        },
    ];
    let tree = BuildStagedOutputTree {
        commitment: commitment_for_retained_entries(&entries).unwrap(),
        entries,
    };
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

#[test]
fn recovery_requires_each_recorded_parent_to_be_a_directory() {
    use super::{
        BuildStagedOutputEntryInput, RetainedStagedOutputEntry, RetainedStagedOutputEntryKind,
        commitment_for_retained_entries,
    };
    use std::sync::Arc;
    let path = b"docs/notes.txt".to_vec();
    let contents: Arc<[u8]> = Arc::from(b"notes".as_slice());
    let child = RetainedStagedOutputEntry {
        relative_path: path.clone(),
        kind: RetainedStagedOutputEntryKind::File {
            bytes: Arc::clone(&contents),
            executable: false,
        },
    };
    let commitment = commitment_for_retained_entries(&[child]).unwrap();
    let recovered = BuildStagedOutputTree::from_retained_parts(
        &commitment,
        [(
            path,
            BuildStagedOutputEntryInput::File {
                bytes: contents,
                executable: false,
            },
        )],
    );
    assert!(
        recovered.is_err(),
        "recovery accepted a file whose parent directory is missing"
    );
}
