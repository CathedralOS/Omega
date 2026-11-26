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

#[test]
fn selecting_a_directory_retains_data_and_compiles_only_its_regular_omega_files() {
    let source = baseline_tree().select_directory(b"docs").unwrap();
    let names: Vec<_> = source
        .entries()
        .map(|entry| entry.relative_path())
        .collect();
    assert_eq!(
        names,
        [b"inner".as_slice(), b"inner/a.txt", b"inner/gen.omg"]
    );
    let files = source.source_files().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].relative_path(), b"inner/gen.omg");
    assert_eq!(files[0].bytes(), b"data Generated {}\n");
    assert_eq!(
        source.digest(),
        source.select_directory(b"").unwrap().digest()
    );
    assert!(source.select_directory(b"inner/a.txt").is_err());
    assert!(source.select_directory(b"missing").is_err());
}

#[test]
fn rebasing_a_source_directory_preserves_link_confinement() {
    let tree = from_entries(&[
        OutputTreeEntry::directory(b"generated"),
        OutputTreeEntry::regular_file(b"outside.txt", b"external", false),
        OutputTreeEntry::symbolic_link(b"generated/escape.omg", b"../outside.txt"),
    ])
    .unwrap();
    assert!(tree.select_directory(b"generated").is_err());

    let tree = from_entries(&[
        OutputTreeEntry::directory(b"generated"),
        OutputTreeEntry::regular_file(b"generated/data.txt", b"ordinary data", false),
        OutputTreeEntry::symbolic_link(b"generated/alias.omg", b"data.txt"),
    ])
    .unwrap();
    let selected = tree.select_directory(b"generated").unwrap();
    assert!(matches!(
        selected.sealed_entry(b"alias.omg").unwrap().kind(),
        super::BuildStagedOutputEntryKind::Symlink {
            target: b"data.txt"
        }
    ));
    assert!(selected.source_files().unwrap().is_empty());
}
