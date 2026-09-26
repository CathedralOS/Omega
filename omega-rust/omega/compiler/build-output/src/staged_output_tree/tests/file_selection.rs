use super::baseline_tree;
use crate::staged_output_tree::RetainedStagedOutputEntryKind;
use crate::{OutputTreeEntry, empty, from_entries};
use std::sync::Arc;

#[test]
fn verifies_existing_materialization_and_rejects_changed_or_extra_content() {
    let session = super::Session::new();
    let tree = baseline_tree();
    let destination = session.destination("verify-existing");
    tree.materialize_into(&destination).unwrap();
    assert_eq!(
        tree.verify_materialized_at(&destination).unwrap(),
        tree.commitment()
    );

    let artifact = destination.join("report.txt");
    std::fs::write(&artifact, b"changed!!!!").unwrap();
    assert!(tree.verify_materialized_at(&destination).is_err());
    assert_eq!(std::fs::read(&artifact).unwrap(), b"changed!!!!");

    std::fs::write(&artifact, b"report-body").unwrap();
    let extra = destination.join("scratch.txt");
    std::fs::write(&extra, b"scratch").unwrap();
    assert!(tree.verify_materialized_at(&destination).is_err());
    assert!(extra.exists());
}

#[test]
fn selects_only_requested_files_and_ancestors_in_canonical_order() {
    let tree = baseline_tree();
    let selected = tree
        .select_files(&[b"report.txt", b"docs/inner/a.txt"])
        .unwrap();
    let expected = from_entries(&[
        OutputTreeEntry::directory(b"docs"),
        OutputTreeEntry::directory(b"docs/inner"),
        OutputTreeEntry::regular_file(b"docs/inner/a.txt", b"alpha", false),
        OutputTreeEntry::regular_file(b"report.txt", b"report-body", false),
    ])
    .unwrap();
    assert_eq!(selected, expected);
    assert_eq!(
        selected,
        tree.select_files(&[b"docs/inner/a.txt", b"report.txt"])
            .unwrap()
    );
    assert_eq!(
        tree.select_files(&[b"report.txt"]).unwrap().entry_count(),
        1
    );
    assert_eq!(tree.select_files(&[]).unwrap(), empty());
    assert_eq!(tree.entry_count(), 6);
    for selected_entry in &selected.entries {
        let RetainedStagedOutputEntryKind::File {
            bytes: selected_bytes,
            ..
        } = &selected_entry.kind
        else {
            continue;
        };
        let original_entry = tree
            .entries
            .iter()
            .find(|entry| entry.relative_path == selected_entry.relative_path)
            .unwrap();
        let RetainedStagedOutputEntryKind::File {
            bytes: original_bytes,
            ..
        } = &original_entry.kind
        else {
            panic!("original regular file")
        };
        assert!(Arc::ptr_eq(selected_bytes, original_bytes));
    }
}

#[test]
fn rejects_missing_noncanonical_directory_and_duplicate_requests() {
    let tree = baseline_tree();
    for requested in [
        b"missing".as_slice(),
        b"docs",
        b"docs/inner",
        b"./report.txt",
        b"docs/../report.txt",
        b"",
        b"docs\\inner\\a.txt",
    ] {
        assert!(tree.select_files(&[requested]).is_err());
    }
    assert!(tree.select_files(&[b"report.txt", b"report.txt"]).is_err());
    assert!(tree.select_files(&[b"docs/inner/a.txt", b"docs"]).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_links_and_executable_files_without_following_or_changing_them() {
    let tree = from_entries(&[
        OutputTreeEntry::directory(b"files"),
        OutputTreeEntry::regular_file(b"files/artifact", b"ordinary", false),
        OutputTreeEntry::symbolic_link(b"alias", b"files"),
        OutputTreeEntry::symbolic_link(b"link", b"files/artifact"),
        OutputTreeEntry::regular_file(b"tool", b"executable", true),
    ])
    .unwrap();
    for requested in [b"alias".as_slice(), b"alias/artifact", b"link", b"tool"] {
        assert!(tree.select_files(&[requested]).is_err());
    }
    assert_eq!(
        tree.select_files(&[b"files/artifact"])
            .unwrap()
            .entry_count(),
        2
    );
}
