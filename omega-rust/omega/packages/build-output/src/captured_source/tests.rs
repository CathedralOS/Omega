//! Portable captured inventory selection and ordering algorithms.
use super::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind, CapturedBuildSourceInput,
    CapturedSourceEntry, CapturedSourceEntryKind,
};
use filesystem_authority::CanonicalFilesystemMetadataRow;

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
fn capture_subset_compares_exact_bytes_not_only_paths_and_lengths() {
    let complete = input_with_template();
    let subset = |bytes: &[u8]| {
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
                    CapturedSourceEntry::file(bytes.to_vec(), false),
                ),
            ],
        )
        .unwrap()
    };
    assert!(subset(b"HELLO WORLD").is_subset_of(&complete));
    assert!(!subset(b"OTHER BYTES").is_subset_of(&complete));
    assert!(!complete.is_subset_of(&subset(b"HELLO WORLD")));
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
