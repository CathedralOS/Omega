//! Captured source tests.

use super::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind, CapturedBuildSourceInput,
    CapturedSourceEntry, CapturedSourceEntryKind,
};
use checked_interpreter::CanonicalFilesystemMetadataRow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-captured-source-{label}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).expect("create captured-source fixture");
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        clear_sealed_modes(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn clear_sealed_modes(root: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(metadata) = std::fs::symlink_metadata(root) else {
        return;
    };
    if metadata.is_dir() {
        let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755));
        if let Ok(children) = std::fs::read_dir(root) {
            for child in children.flatten() {
                clear_sealed_modes(&child.path());
            }
        }
    } else {
        let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o644));
    }
}

#[cfg(not(unix))]
fn clear_sealed_modes(_root: &Path) {}

fn index(rows: &[(&[u8], CanonicalFilesystemMetadataRowKind)]) -> CanonicalFilesystemMetadataIndex {
    CanonicalFilesystemMetadataIndex::version_1(
        [0x42; 32],
        rows.iter()
            .map(|(path, kind)| CanonicalFilesystemMetadataRow::new(*path, *kind)),
    )
    .expect("construct canonical source metadata")
}

fn input_with_template() -> CapturedBuildSourceInput {
    CapturedBuildSourceInput::from_capture_rows(
        index(&[
            (b"", CanonicalFilesystemMetadataRowKind::Directory),
            (b"templates", CanonicalFilesystemMetadataRowKind::Directory),
            (
                b"templates/banner.tmpl",
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 11,
                },
            ),
            (
                b"tools/run.sh",
                CanonicalFilesystemMetadataRowKind::File {
                    executable: true,
                    logical_byte_length: 3,
                },
            ),
            (b"tools", CanonicalFilesystemMetadataRowKind::Directory),
            (
                b"link",
                CanonicalFilesystemMetadataRowKind::Symlink {
                    target_spelling_logical_byte_length: 9,
                },
            ),
        ]),
        [
            (b"templates".to_vec(), CapturedSourceEntry::Directory),
            (
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::file(b"HELLO WORLD".to_vec(), false),
            ),
            (b"tools".to_vec(), CapturedSourceEntry::Directory),
            (
                b"tools/run.sh".to_vec(),
                CapturedSourceEntry::file(b"RUN".to_vec(), true),
            ),
            (
                b"link".to_vec(),
                CapturedSourceEntry::symlink(b"templates".to_vec()),
            ),
        ],
    )
    .expect("assemble captured build source input")
}

#[test]
fn captured_inventory_lists_entries_in_deterministic_canonical_order() {
    let input = input_with_template();
    assert_eq!(input.entry_count(), 6, "inventory includes the root row");
    assert_eq!(input.file_bytes(), 14);
    let listed = input
        .entries()
        .map(|entry| {
            (
                entry.relative_path().to_vec(),
                match entry.kind() {
                    CapturedSourceEntryKind::Directory => "dir",
                    CapturedSourceEntryKind::File { .. } => "file",
                    CapturedSourceEntryKind::Symlink { .. } => "link",
                },
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        listed,
        vec![
            (b"link".to_vec(), "link"),
            (b"templates".to_vec(), "dir"),
            (b"templates/banner.tmpl".to_vec(), "file"),
            (b"tools".to_vec(), "dir"),
            (b"tools/run.sh".to_vec(), "file"),
        ],
        "entries list in unsigned canonical byte order"
    );
    let template = input
        .file(b"templates/banner.tmpl")
        .expect("exact captured file lookup");
    assert_eq!(template.bytes(), b"HELLO WORLD");
    assert!(!template.executable());
    assert!(
        input.file(b"link").is_none(),
        "a captured link is inert data, not a file"
    );
    assert!(input.file(b"absent").is_none());
}

#[test]
fn captured_inventory_rejects_shape_disagreement_with_its_index() {
    // Entry path absent from the index.
    assert!(
        CapturedBuildSourceInput::from_capture_rows(
            index(&[(b"", CanonicalFilesystemMetadataRowKind::Directory)]),
            [(b"extra".to_vec(), CapturedSourceEntry::Directory)],
        )
        .is_err(),
        "entries outside the canonical index reject"
    );
    // Index row without a retained entry.
    assert!(
        CapturedBuildSourceInput::from_capture_rows(
            index(&[
                (b"", CanonicalFilesystemMetadataRowKind::Directory),
                (b"missing", CanonicalFilesystemMetadataRowKind::Directory),
            ]),
            Vec::new(),
        )
        .is_err(),
        "omitted index rows reject"
    );
    // Kind disagreement.
    assert!(
        CapturedBuildSourceInput::from_capture_rows(
            index(&[
                (b"", CanonicalFilesystemMetadataRowKind::Directory),
                (
                    b"name",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                ),
            ]),
            [(b"name".to_vec(), CapturedSourceEntry::Directory)],
        )
        .is_err(),
        "kind disagreement rejects"
    );
    // Extent disagreement.
    assert!(
        CapturedBuildSourceInput::from_capture_rows(
            index(&[
                (b"", CanonicalFilesystemMetadataRowKind::Directory),
                (
                    b"name",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 9,
                    },
                ),
            ]),
            [(
                b"name".to_vec(),
                CapturedSourceEntry::file(b"tiny".to_vec(), false)
            )],
        )
        .is_err(),
        "logical length disagreement rejects"
    );
    // Noncanonical entry path. The index constructor already refuses
    // noncanonical rows, so the entry carries the bad spelling against an
    // otherwise valid index and `from_capture_rows` applies its own check.
    assert!(
        CapturedBuildSourceInput::from_capture_rows(
            index(&[(b"", CanonicalFilesystemMetadataRowKind::Directory)]),
            [(
                b"../escape".to_vec(),
                CapturedSourceEntry::file(Vec::new(), false)
            )],
        )
        .is_err(),
        "a noncanonical retained path rejects even when the index was forged"
    );
}

/// A portable inventory: no executable class and no symlink, so every
/// supported host can materialize it.
fn ordinary_input() -> CapturedBuildSourceInput {
    CapturedBuildSourceInput::from_capture_rows(
        index(&[
            (b"", CanonicalFilesystemMetadataRowKind::Directory),
            (b"templates", CanonicalFilesystemMetadataRowKind::Directory),
            (
                b"templates/banner.tmpl",
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 11,
                },
            ),
        ]),
        [
            (b"templates".to_vec(), CapturedSourceEntry::Directory),
            (
                b"templates/banner.tmpl".to_vec(),
                CapturedSourceEntry::file(b"HELLO WORLD".to_vec(), false),
            ),
        ],
    )
    .expect("assemble ordinary captured build source input")
}

#[test]
fn materialized_snapshot_reinspects_exact_kinds_bytes_and_modes() {
    let input = ordinary_input();
    let fixture = Fixture::new("materialize");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");
    input
        .materialize_into(&backing)
        .expect("materialize captured source input");

    let template = backing.join("templates/banner.tmpl");
    assert_eq!(
        std::fs::read(&template).expect("read materialized template"),
        b"HELLO WORLD"
    );
    assert!(
        backing.join("templates").is_dir(),
        "captured directories materialize as concrete directories"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::symlink_metadata(&template)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
        assert_eq!(
            std::fs::symlink_metadata(backing.join("templates"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o555
        );
    }
    #[cfg(not(unix))]
    {
        assert!(
            std::fs::symlink_metadata(&template)
                .unwrap()
                .permissions()
                .readonly(),
            "the readonly attribute is the materialized file seal here"
        );
    }
}

#[cfg(unix)]
#[test]
fn materialized_snapshot_reinspects_executable_modes_and_inert_links() {
    use std::os::unix::fs::PermissionsExt;

    let input = input_with_template();
    let fixture = Fixture::new("materialize-modes");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");
    input
        .materialize_into(&backing)
        .expect("materialize captured source input");

    assert_eq!(
        std::fs::read(backing.join("tools/run.sh")).expect("read materialized tool"),
        b"RUN"
    );
    assert!(
        !backing.join("link").exists(),
        "captured links materialize as inert evidence only"
    );
    assert_eq!(
        std::fs::symlink_metadata(backing.join("tools/run.sh"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o555
    );
}

#[cfg(not(unix))]
#[test]
fn materialization_rejects_an_unrepresentable_executable_mode() {
    let input = input_with_template();
    let fixture = Fixture::new("executable-mode");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");

    let error = input
        .materialize_into(&backing)
        .expect_err("this host cannot represent a captured executable mode");
    assert!(
        error.message().contains("executable mode"),
        "unexpected materialization error: {}",
        error.message()
    );
    assert!(
        std::fs::read_dir(&backing)
            .expect("enumerate rejected backing")
            .next()
            .is_none(),
        "an unrepresentable inventory rejects before any write"
    );
}

#[test]
fn materialized_snapshot_rejects_a_nonempty_destination() {
    let input = input_with_template();
    let fixture = Fixture::new("occupied");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create backing");
    std::fs::write(backing.join("stray"), b"x").expect("write stray entry");
    assert!(
        input.materialize_into(&backing).is_err(),
        "a non-empty backing is not a fresh private snapshot"
    );
}
