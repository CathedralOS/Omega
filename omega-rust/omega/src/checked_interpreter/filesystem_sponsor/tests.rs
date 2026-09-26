//! Filesystem sponsor tests.

use super::{
    FilesystemSponsor, FilesystemSponsorEntry, FilesystemSponsorError, FilesystemSponsorLimits,
    FilesystemSponsorNamespaceEntryKind, FilesystemSponsorPath, FilesystemSponsorSnapshot, Path,
};

fn limits(entries: u64, total: u64, extent: u64) -> FilesystemSponsorLimits {
    FilesystemSponsorLimits {
        maximum_entries: entries,
        maximum_total_logical_bytes: total,
        maximum_object_extent: extent,
    }
}

fn sponsor_root() -> &'static Path {
    #[cfg(windows)]
    {
        Path::new(r"C:\staging\session")
    }
    #[cfg(not(windows))]
    {
        Path::new("/staging/session")
    }
}

fn other_sponsor_root() -> &'static Path {
    #[cfg(windows)]
    {
        Path::new(r"C:\other\session")
    }
    #[cfg(not(windows))]
    {
        Path::new("/other/session")
    }
}

fn sponsor_with(limits: FilesystemSponsorLimits) -> FilesystemSponsor {
    FilesystemSponsor::with_limits(sponsor_root(), limits).unwrap()
}

fn path(sponsor: &FilesystemSponsor, suffix: &str) -> FilesystemSponsorPath {
    sponsor.bind_path(sponsor_root().join(suffix)).unwrap()
}

fn commit_directory(sponsor: &FilesystemSponsor, suffix: &str) {
    sponsor
        .prepare_create_directory(&path(sponsor, suffix))
        .unwrap()
        .commit()
        .unwrap();
}

fn commit_object(sponsor: &FilesystemSponsor, suffix: &str, extent: u64) -> FilesystemSponsorPath {
    let path = path(sponsor, suffix);
    sponsor
        .prepare_create_object(&path, extent)
        .unwrap()
        .commit()
        .unwrap();
    path
}

#[test]
fn compiler_defaults_are_explicit_and_separate() {
    assert_eq!(FilesystemSponsorLimits::default().maximum_entries, 4_096);
    assert_eq!(
        FilesystemSponsorLimits::default().maximum_total_logical_bytes,
        256 * 1024 * 1024
    );
    assert_eq!(
        FilesystemSponsorLimits::default().maximum_object_extent,
        256 * 1024 * 1024
    );
}

#[test]
fn session_root_is_excluded_and_outside_paths_are_rejected() {
    let sponsor = sponsor_with(limits(2, 2, 2));
    assert_eq!(
        sponsor.bind_path(sponsor_root()).unwrap_err(),
        FilesystemSponsorError::SessionRootIsNotAnEntry
    );
    let outside = sponsor_root()
        .parent()
        .expect("session root has a parent")
        .join("elsewhere");
    assert!(matches!(
        sponsor.bind_path(outside),
        Err(FilesystemSponsorError::PathOutsideSessionRoot(_))
    ));
    assert!(matches!(
        sponsor.bind_path("relative"),
        Err(FilesystemSponsorError::PathMustBeAbsolute(_))
    ));
    assert_eq!(sponsor.snapshot().unwrap().entries, 0);
}

#[test]
fn entry_total_and_extent_limits_fail_during_prepare_without_committing() {
    let sponsor = sponsor_with(limits(2, 7, 5));
    commit_object(&sponsor, "first", 5);
    sponsor
        .prepare_create_symlink(&path(&sponsor, "link"), b"xy")
        .unwrap()
        .commit()
        .unwrap();

    assert!(matches!(
        sponsor.prepare_create_directory(&path(&sponsor, "third")),
        Err(FilesystemSponsorError::EntryLimitExceeded {
            limit: 2,
            attempted: 3
        })
    ));
    sponsor
        .prepare_unlink(&path(&sponsor, "link"))
        .unwrap()
        .commit()
        .unwrap();
    assert!(matches!(
        sponsor.prepare_create_object(&path(&sponsor, "too-large"), 6),
        Err(FilesystemSponsorError::ObjectExtentLimitExceeded {
            limit: 5,
            attempted: 6
        })
    ));

    assert!(matches!(
        sponsor.prepare_create_symlink(&path(&sponsor, "bytes"), b"xyz"),
        Err(FilesystemSponsorError::TotalLogicalBytesLimitExceeded {
            limit: 7,
            attempted: 8
        })
    ));
    assert_eq!(
        sponsor.snapshot().unwrap(),
        FilesystemSponsorSnapshot {
            entries: 1,
            total_logical_bytes: 5,
            unique_objects: 1,
            open_descriptors: 0,
        }
    );
}

#[test]
fn hard_links_add_names_and_entries_but_not_logical_bytes() {
    let sponsor = sponsor_with(limits(4, 10, 10));
    let first = commit_object(&sponsor, "first", 7);
    let second = path(&sponsor, "second");
    sponsor
        .prepare_hard_link(&first, &second)
        .unwrap()
        .commit()
        .unwrap();

    assert_eq!(
        sponsor.entry(&first).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 7,
            names: 2,
            open_descriptors: 0,
        })
    );
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 7);
    sponsor.prepare_unlink(&first).unwrap().commit().unwrap();
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 7);
    assert_eq!(
        sponsor.entry(&second).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 7,
            names: 1,
            open_descriptors: 0,
        })
    );
}

#[test]
fn namespace_snapshot_exposes_groups_extents_and_quiescence() {
    let sponsor = sponsor_with(limits(5, 20, 20));
    commit_directory(&sponsor, "output");
    let first = commit_object(&sponsor, "output/first", 7);
    let second = path(&sponsor, "output/second");
    sponsor
        .prepare_hard_link(&first, &second)
        .unwrap()
        .commit()
        .unwrap();

    let descriptor = sponsor.prepare_open(&first).unwrap().commit().unwrap();
    let snapshot = sponsor.namespace_snapshot().unwrap();
    assert_eq!(snapshot.open_descriptors(), 1);
    assert!(!snapshot.transaction_prepared());
    assert_eq!(snapshot.entries().len(), 3);
    let groups = snapshot
        .entries()
        .iter()
        .filter_map(|entry| match entry.kind() {
            FilesystemSponsorNamespaceEntryKind::Object { group, extent } => {
                assert_eq!(extent, 7);
                Some(group)
            }
            FilesystemSponsorNamespaceEntryKind::Directory => None,
            FilesystemSponsorNamespaceEntryKind::Symlink { .. } => {
                panic!("fixture has no symlink")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0], groups[1]);

    sponsor
        .prepare_close(&descriptor)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(sponsor.namespace_snapshot().unwrap().open_descriptors(), 0);
}

#[test]
fn rename_replacement_retains_an_open_replaced_object_until_close() {
    let sponsor = sponsor_with(limits(4, 20, 20));
    let source = commit_object(&sponsor, "source", 3);
    let destination = commit_object(&sponsor, "destination", 5);
    let replaced_descriptor = sponsor
        .prepare_open(&destination)
        .unwrap()
        .commit()
        .unwrap();

    sponsor
        .prepare_rename(&source, &destination)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        sponsor.snapshot().unwrap(),
        FilesystemSponsorSnapshot {
            entries: 1,
            total_logical_bytes: 8,
            unique_objects: 2,
            open_descriptors: 1,
        }
    );
    assert_eq!(
        sponsor.entry(&destination).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 3,
            names: 1,
            open_descriptors: 0,
        })
    );

    sponsor
        .prepare_close(&replaced_descriptor)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 3);
    assert_eq!(sponsor.snapshot().unwrap().unique_objects, 1);
}

#[test]
fn open_unlink_close_keeps_unnamed_object_charged_until_close() {
    let sponsor = sponsor_with(limits(2, 20, 20));
    let object = commit_object(&sponsor, "object", 10);
    let descriptor = sponsor.prepare_open(&object).unwrap().commit().unwrap();
    sponsor.prepare_unlink(&object).unwrap().commit().unwrap();

    assert_eq!(
        sponsor.snapshot().unwrap(),
        FilesystemSponsorSnapshot {
            entries: 0,
            total_logical_bytes: 10,
            unique_objects: 1,
            open_descriptors: 1,
        }
    );
    sponsor
        .prepare_close(&descriptor)
        .unwrap()
        .commit()
        .unwrap();
    assert_eq!(
        sponsor.snapshot().unwrap(),
        FilesystemSponsorSnapshot {
            entries: 0,
            total_logical_bytes: 0,
            unique_objects: 0,
            open_descriptors: 0,
        }
    );
}

#[test]
fn create_object_open_commits_namespace_object_and_descriptor_together() {
    let sponsor = sponsor_with(limits(2, 20, 20));
    let object = path(&sponsor, "created-open");
    let prepared = sponsor.prepare_create_object_open(&object, 9).unwrap();

    assert_eq!(sponsor.entry(&object).unwrap(), None);
    assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 0);
    let descriptor = prepared.commit().unwrap();
    assert_eq!(
        sponsor.entry(&object).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 9,
            names: 1,
            open_descriptors: 1,
        })
    );
    assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 1);

    sponsor
        .prepare_close(&descriptor)
        .unwrap()
        .commit()
        .unwrap();
}

#[test]
fn open_with_extent_commits_truncation_and_descriptor_together() {
    let sponsor = sponsor_with(limits(2, 20, 20));
    let object = commit_object(&sponsor, "truncate-open", 13);
    let prepared = sponsor.prepare_open_with_extent(&object, Some(0)).unwrap();

    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 13);
    let descriptor = prepared.commit().unwrap();
    assert_eq!(
        sponsor.entry(&object).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 0,
            names: 1,
            open_descriptors: 1,
        })
    );
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 0);

    sponsor
        .prepare_close(&descriptor)
        .unwrap()
        .commit()
        .unwrap();
}

#[test]
fn duplicate_commits_a_second_descriptor_for_the_same_unique_object() {
    let sponsor = sponsor_with(limits(2, 20, 20));
    let object = commit_object(&sponsor, "duplicate", 11);
    let first = sponsor.prepare_open(&object).unwrap().commit().unwrap();
    let prepared = sponsor.prepare_duplicate(&first).unwrap();

    assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 1);
    let second = prepared.commit().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        sponsor.entry(&object).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 11,
            names: 1,
            open_descriptors: 2,
        })
    );
    assert_eq!(sponsor.snapshot().unwrap().unique_objects, 1);
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 11);

    sponsor.prepare_unlink(&object).unwrap().commit().unwrap();
    sponsor.prepare_close(&first).unwrap().commit().unwrap();
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 11);
    sponsor.prepare_close(&second).unwrap().commit().unwrap();
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 0);
}

#[test]
fn partial_write_reserves_requested_growth_but_commits_actual_growth() {
    let sponsor = sponsor_with(limits(2, 10, 10));
    let object = commit_object(&sponsor, "object", 2);
    let descriptor = sponsor.prepare_open(&object).unwrap().commit().unwrap();

    sponsor
        .prepare_write(&descriptor, 2, 8)
        .unwrap()
        .commit_written(3)
        .unwrap();
    assert_eq!(
        sponsor.entry(&object).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 5,
            names: 1,
            open_descriptors: 1,
        })
    );
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 5);

    sponsor
        .prepare_write(&descriptor, 9, 0)
        .unwrap()
        .commit_written(0)
        .unwrap();
    assert_eq!(
        sponsor.entry(&object).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 5,
            names: 1,
            open_descriptors: 1,
        }),
        "a zero-byte write beyond EOF must not extend the sponsored object"
    );

    let prepared = sponsor.prepare_write(&descriptor, 5, 5).unwrap();
    assert_eq!(
        prepared.commit_written(6).unwrap_err(),
        FilesystemSponsorError::PartialWriteExceedsPrepared {
            prepared: 5,
            actual: 6,
        }
    );
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 5);
}

#[test]
fn failed_or_aborted_provider_mutation_never_commits_candidate_state() {
    let sponsor = sponsor_with(limits(4, 20, 20));
    let abandoned = path(&sponsor, "abandoned");
    let prepared = sponsor.prepare_create_object(&abandoned, 12).unwrap();
    assert_eq!(sponsor.snapshot().unwrap().entries, 0);
    assert_eq!(
        sponsor
            .prepare_create_directory(&path(&sponsor, "blocked"))
            .unwrap_err(),
        FilesystemSponsorError::TransactionAlreadyPrepared
    );
    prepared.abort();
    assert_eq!(sponsor.snapshot().unwrap().entries, 0);

    commit_directory(&sponsor, "committed");
    assert_eq!(sponsor.snapshot().unwrap().entries, 1);
    assert_eq!(sponsor.entry(&abandoned).unwrap(), None);
}

#[test]
fn arithmetic_overflow_is_rejected_during_write_prepare() {
    let sponsor = sponsor_with(limits(2, u64::MAX, u64::MAX));
    let object = commit_object(&sponsor, "object", 0);
    let descriptor = sponsor.prepare_open(&object).unwrap().commit().unwrap();
    assert_eq!(
        sponsor.prepare_write(&descriptor, u64::MAX, 1).unwrap_err(),
        FilesystemSponsorError::ArithmeticOverflow
    );
    assert_eq!(sponsor.snapshot().unwrap().total_logical_bytes, 0);
}

#[test]
fn account_bound_paths_and_descriptors_reject_cross_account_operations() {
    let first = sponsor_with(limits(4, 20, 20));
    let second = FilesystemSponsor::with_limits(other_sponsor_root(), limits(4, 20, 20)).unwrap();
    let first_object = commit_object(&first, "object", 1);
    let first_descriptor = first.prepare_open(&first_object).unwrap().commit().unwrap();
    let second_name = second.bind_path(other_sponsor_root().join("name")).unwrap();

    assert_eq!(
        second
            .prepare_hard_link(&first_object, &second_name)
            .unwrap_err(),
        FilesystemSponsorError::CrossAccountOperation
    );
    assert_eq!(
        second
            .prepare_rename(&first_object, &second_name)
            .unwrap_err(),
        FilesystemSponsorError::CrossAccountOperation
    );
    assert_eq!(
        second.prepare_close(&first_descriptor).unwrap_err(),
        FilesystemSponsorError::CrossAccountOperation
    );
    assert_eq!(second.snapshot().unwrap().entries, 0);
}

#[test]
fn directory_rename_moves_its_complete_namespace_subtree() {
    let sponsor = sponsor_with(limits(5, 10, 10));
    commit_directory(&sponsor, "old");
    let child = commit_object(&sponsor, "old/child", 4);
    let old = path(&sponsor, "old");
    let new = path(&sponsor, "new");
    sponsor
        .prepare_rename(&old, &new)
        .unwrap()
        .commit()
        .unwrap();

    assert_eq!(sponsor.entry(&old).unwrap(), None);
    assert_eq!(sponsor.entry(&child).unwrap(), None);
    assert_eq!(
        sponsor.entry(&path(&sponsor, "new/child")).unwrap(),
        Some(FilesystemSponsorEntry::Object {
            extent: 4,
            names: 1,
            open_descriptors: 0,
        })
    );
    assert_eq!(sponsor.snapshot().unwrap().entries, 2);
}
