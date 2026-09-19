//! Sponsor and provider tests for the real filesystem: grants, canonical paths, locks and metadata.

use super::{
    DirectoryEntrySnapshotKind, EACCES, Path, PathBuf, RealFs, SponsorPreparation,
    canonical_grants, canonical_metadata_values, canonical_relative_path,
    read_only_open_bypasses_sponsor, real_directory_entries, resolve_for_check,
    sponsor_preparation,
};
#[cfg(unix)]
use super::{real_path, resolve_parent_for_check};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let identity = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-real-fs-{label}-{}-{identity}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create provider test directory");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn test_grant_root(identity: u32, path: PathBuf) -> crate::FilesystemGrantRoot {
    crate::FilesystemGrantRoot::new(
        crate::FilesystemGrantRootIdentity::new(identity).expect("test grant identity is nonzero"),
        path,
    )
}

#[test]
fn real_directory_listing_keeps_host_failures_in_the_errno_lane() {
    let directory = TestDirectory::new("directory-errno");
    let missing = directory.0.join("missing");

    for snapshot_kind in [
        DirectoryEntrySnapshotKind::PackedRecords,
        DirectoryEntrySnapshotKind::FindCursor,
    ] {
        match real_directory_entries(&missing, snapshot_kind) {
            Ok(Err(errno)) => assert_ne!(errno, 0),
            Ok(Ok(_)) => panic!("missing directory cannot enumerate"),
            Err(_) => panic!("ordinary host failure is not a resource halt"),
        }
    }
}

#[test]
fn grant_root_identities_and_physical_roots_are_unambiguous() {
    let directory = TestDirectory::new("grant-identities");
    let source = directory.0.join("source");
    let output = directory.0.join("output");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&output).unwrap();

    let duplicate_identity = canonical_grants(crate::FsGrants {
        read_roots: vec![test_grant_root(1, source.clone())],
        write_roots: vec![test_grant_root(1, output.clone())],
    })
    .expect_err("one evidence identity cannot name two roots");
    assert!(duplicate_identity.contains("identity `1` is duplicated"));

    let duplicate_physical = canonical_grants(crate::FsGrants {
        read_roots: vec![test_grant_root(1, source.clone())],
        write_roots: vec![test_grant_root(2, source)],
    })
    .expect_err("one physical root cannot carry two evidence identities");
    assert!(duplicate_physical.contains("conflicting identities `1` and `2`"));
}

#[test]
fn nested_output_root_is_selected_independently_of_grant_order() {
    let directory = TestDirectory::new("nested-grant");
    let source = directory.0.join("source");
    let output = source.join("build");
    std::fs::create_dir_all(&output).unwrap();
    let artifact = output.join("artifact.bin");

    let grants = canonical_grants(crate::FsGrants {
        read_roots: vec![test_grant_root(1, source)],
        write_roots: vec![test_grant_root(2, output)],
    })
    .unwrap();
    let resolved_artifact = resolve_for_check(&artifact).unwrap();
    let selected = grants
        .matching_root(&resolved_artifact, false)
        .expect("nested output is readable through the write root");
    assert_eq!(selected.identity.get(), 2);
    assert_eq!(
        canonical_relative_path(resolved_artifact.strip_prefix(&selected.path).unwrap()).unwrap(),
        b"artifact.bin"
    );
}

#[cfg(unix)]
#[test]
fn non_utf8_path_bytes_are_not_lossily_rewritten() {
    use std::os::unix::ffi::OsStrExt;

    let bytes = b"raw-\xff-name";
    let path = real_path(bytes).expect("unix paths preserve arbitrary bytes");
    assert_eq!(path.as_os_str().as_bytes(), bytes);
    assert!(
        canonical_relative_path(&path).is_none(),
        "rooted evidence must reject a path it cannot encode losslessly"
    );
}

#[test]
fn source_reads_and_the_session_root_bypass_sponsorship() {
    let root = Path::new("/staging/session");
    assert!(read_only_open_bypasses_sponsor(
        Path::new("/sources/package/input.txt"),
        root,
        false,
        false,
        false,
    ));
    assert!(read_only_open_bypasses_sponsor(
        root, root, false, false, false,
    ));
    assert!(!read_only_open_bypasses_sponsor(
        Path::new("/staging/session/output.txt"),
        root,
        false,
        false,
        false,
    ));
    assert!(!read_only_open_bypasses_sponsor(
        Path::new("/sources/package/input.txt"),
        root,
        false,
        false,
        true,
    ));
}

#[test]
fn ordinary_namespace_preconditions_are_deferred_to_the_host() {
    let directory = TestDirectory::new("host-precondition");
    let sponsor = crate::FilesystemSponsor::new(&directory.0).unwrap();
    let child = sponsor
        .bind_path(directory.0.join("missing/child"))
        .unwrap();
    assert!(matches!(
        sponsor_preparation(sponsor.prepare_create_directory(&child)),
        Ok(SponsorPreparation::ExpectedHostFailure)
    ));
}

#[test]
fn real_fs_drop_closes_descriptors_and_preserves_named_charges() {
    let directory = TestDirectory::new("drop-close");
    let path = directory.0.join("output.bin");
    std::fs::write(&path, b"1234567").unwrap();
    let sponsor = crate::FilesystemSponsor::new(&directory.0).unwrap();
    let sponsored_path = sponsor.bind_path(&path).unwrap();
    let descriptor = sponsor
        .prepare_create_object_open(&sponsored_path, 7)
        .unwrap()
        .commit()
        .unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let mut filesystem = RealFs::new(None, Some(sponsor.clone())).unwrap();
    filesystem
        .insert(file, path, Some(descriptor), false)
        .unwrap();
    assert_eq!(sponsor.snapshot().unwrap().open_descriptors, 1);

    drop(filesystem);

    let snapshot = sponsor.snapshot().unwrap();
    assert_eq!(snapshot.open_descriptors, 0);
    assert_eq!(snapshot.entries, 1);
    assert_eq!(snapshot.unique_objects, 1);
    assert_eq!(snapshot.total_logical_bytes, 7);
}

#[test]
fn resource_halt_teardown_closes_remaining_descriptors() {
    let directory = TestDirectory::new("resource-close");
    let path = directory.0.join("output.bin");
    std::fs::write(&path, []).unwrap();
    let sponsor = crate::FilesystemSponsor::with_limits(
        &directory.0,
        crate::FilesystemSponsorLimits {
            maximum_entries: 1,
            maximum_total_logical_bytes: 0,
            maximum_object_extent: 0,
        },
    )
    .unwrap();
    let sponsored_path = sponsor.bind_path(&path).unwrap();
    let descriptor = sponsor
        .prepare_create_object_open(&sponsored_path, 0)
        .unwrap()
        .commit()
        .unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let mut filesystem = RealFs::new(None, Some(sponsor.clone())).unwrap();
    filesystem
        .insert(file, path, Some(descriptor), false)
        .unwrap();

    assert!(matches!(
        sponsor_preparation(sponsor.prepare_write(&descriptor, 0, 1)),
        Err(super::super::Halt::Resource(_))
    ));
    drop(filesystem);

    let snapshot = sponsor.snapshot().unwrap();
    assert_eq!(snapshot.open_descriptors, 0);
    assert_eq!(snapshot.entries, 1);
    assert_eq!(snapshot.total_logical_bytes, 0);
}

fn canonical_metadata(
    rows: impl IntoIterator<Item = crate::CanonicalFilesystemMetadataRow>,
) -> crate::CanonicalFilesystemMetadataIndex {
    crate::CanonicalFilesystemMetadataIndex::version_1([7; 32], rows).unwrap()
}

fn canonical_grant_root(
    identity: u32,
    path: PathBuf,
    rows: impl IntoIterator<Item = crate::CanonicalFilesystemMetadataRow>,
) -> crate::FilesystemGrantRoot {
    test_grant_root(identity, path).with_canonical_metadata(canonical_metadata(rows))
}

#[test]
fn canonical_metadata_has_exact_root_directory_file_executable_and_symlink_values() {
    use crate::CanonicalFilesystemMetadataRowKind::{Directory, File, Symlink};

    assert_eq!(
        canonical_metadata_values(Directory),
        (0o040555, 0, 1_000_000_000)
    );
    assert_eq!(
        canonical_metadata_values(File {
            executable: false,
            logical_byte_length: 17,
        }),
        (0o100444, 17, 1_000_000_000)
    );
    assert_eq!(
        canonical_metadata_values(File {
            executable: true,
            logical_byte_length: 23,
        }),
        (0o100555, 23, 1_000_000_000)
    );
    assert_eq!(
        canonical_metadata_values(Symlink {
            target_spelling_logical_byte_length: 11,
        }),
        (0o120777, 11, 1_000_000_000)
    );
}

#[cfg(unix)]
#[test]
fn canonical_stat_and_lstat_select_target_and_authored_leaf_rows() {
    use crate::{CanonicalFilesystemMetadataRow, CanonicalFilesystemMetadataRowKind};
    use std::os::unix::fs::symlink;

    let directory = TestDirectory::new("canonical-stat-lstat");
    let target = directory.0.join("target.bin");
    let link = directory.0.join("alias");
    std::fs::write(&target, b"data").unwrap();
    symlink("target.bin", &link).unwrap();
    let grants = crate::FsGrants {
        read_roots: vec![canonical_grant_root(
            1,
            directory.0.clone(),
            [
                CanonicalFilesystemMetadataRow::new(
                    b"".to_vec(),
                    CanonicalFilesystemMetadataRowKind::Directory,
                ),
                CanonicalFilesystemMetadataRow::new(
                    b"target.bin".to_vec(),
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 4,
                    },
                ),
                CanonicalFilesystemMetadataRow::new(
                    b"alias".to_vec(),
                    CanonicalFilesystemMetadataRowKind::Symlink {
                        target_spelling_logical_byte_length: 10,
                    },
                ),
            ],
        )],
        write_roots: Vec::new(),
    };
    let filesystem = RealFs::new(Some(grants), None).unwrap();

    let followed = resolve_for_check(&link).unwrap();
    assert_eq!(followed, target.canonicalize().unwrap());
    assert_eq!(
        filesystem.canonical_metadata_for_path(&followed),
        Ok(Some(CanonicalFilesystemMetadataRowKind::File {
            executable: false,
            logical_byte_length: 4,
        }))
    );

    let authored_leaf = resolve_parent_for_check(&link).unwrap();
    assert_eq!(
        authored_leaf,
        directory.0.canonicalize().unwrap().join("alias")
    );
    assert_eq!(
        filesystem.canonical_metadata_for_path(&authored_leaf),
        Ok(Some(CanonicalFilesystemMetadataRowKind::Symlink {
            target_spelling_logical_byte_length: 10,
        }))
    );
    assert_eq!(
        filesystem.canonical_metadata_for_path(&directory.0.canonicalize().unwrap()),
        Ok(Some(CanonicalFilesystemMetadataRowKind::Directory))
    );
}

#[test]
fn canonical_fstat_retains_the_row_selected_when_descriptor_was_opened() {
    use crate::{CanonicalFilesystemMetadataRow, CanonicalFilesystemMetadataRowKind};

    let directory = TestDirectory::new("canonical-fstat");
    let path = directory.0.join("input.bin");
    std::fs::write(&path, b"old").unwrap();
    let grants = crate::FsGrants {
        read_roots: vec![canonical_grant_root(
            1,
            directory.0.clone(),
            [
                CanonicalFilesystemMetadataRow::new(
                    b"".to_vec(),
                    CanonicalFilesystemMetadataRowKind::Directory,
                ),
                CanonicalFilesystemMetadataRow::new(
                    b"input.bin".to_vec(),
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 3,
                    },
                ),
            ],
        )],
        write_roots: Vec::new(),
    };
    let mut filesystem = RealFs::new(Some(grants), None).unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let canonical_path = path.canonicalize().unwrap();
    let fd = filesystem
        .insert(file, canonical_path, None, false)
        .unwrap() as i32;

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert_eq!(
        filesystem.files.get(&fd).unwrap().canonical_metadata,
        Some(CanonicalFilesystemMetadataRowKind::File {
            executable: false,
            logical_byte_length: 3,
        })
    );
}

#[test]
fn canonical_metadata_missing_row_refuses_without_physical_fallback() {
    use crate::{CanonicalFilesystemMetadataRow, CanonicalFilesystemMetadataRowKind};

    let directory = TestDirectory::new("canonical-missing");
    let path = directory.0.join("unbound.bin");
    std::fs::write(&path, b"host bytes").unwrap();
    let grants = crate::FsGrants {
        read_roots: vec![canonical_grant_root(
            1,
            directory.0.clone(),
            [CanonicalFilesystemMetadataRow::new(
                b"".to_vec(),
                CanonicalFilesystemMetadataRowKind::Directory,
            )],
        )],
        write_roots: Vec::new(),
    };
    let mut filesystem = RealFs::new(Some(grants), None).unwrap();
    let canonical_path = path.canonicalize().unwrap();
    assert_eq!(
        filesystem.canonical_metadata_for_path(&canonical_path),
        Err(EACCES)
    );
    let file = std::fs::File::open(&path).unwrap();
    assert_eq!(
        filesystem.insert(file, canonical_path, None, false),
        Err(EACCES)
    );
}

#[test]
fn ordinary_grant_root_keeps_physical_metadata_policy() {
    let directory = TestDirectory::new("physical-metadata");
    let path = directory.0.join("ordinary.bin");
    std::fs::write(&path, b"host bytes").unwrap();
    let grants = crate::FsGrants {
        read_roots: vec![test_grant_root(1, directory.0.clone())],
        write_roots: Vec::new(),
    };
    let mut filesystem = RealFs::new(Some(grants), None).unwrap();
    let canonical_path = path.canonicalize().unwrap();
    assert_eq!(
        filesystem.canonical_metadata_for_path(&canonical_path),
        Ok(None)
    );
    let file = std::fs::File::open(&path).unwrap();
    let fd = filesystem
        .insert(file, canonical_path, None, false)
        .unwrap() as i32;
    assert_eq!(filesystem.files.get(&fd).unwrap().canonical_metadata, None);
}
