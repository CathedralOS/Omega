//! Canonical filesystem metadata tests.

use super::{
    CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT, CanonicalFilesystemMetadataIndex,
    CanonicalFilesystemMetadataIndexError, CanonicalFilesystemMetadataRow,
    CanonicalFilesystemMetadataRowKind,
};

pub(crate) fn row(
    path: &[u8],
    kind: CanonicalFilesystemMetadataRowKind,
) -> CanonicalFilesystemMetadataRow {
    CanonicalFilesystemMetadataRow::new(path.to_vec(), kind)
}

#[test]
pub(crate) fn canonical_metadata_accepts_raw_non_utf8_paths_and_preserves_ordered_rows() {
    let index = CanonicalFilesystemMetadataIndex::version_1(
        [3; 32],
        [
            row(b"raw-\xff", CanonicalFilesystemMetadataRowKind::Directory),
            row(b"a:b", CanonicalFilesystemMetadataRowKind::Directory),
            row(b"", CanonicalFilesystemMetadataRowKind::Directory),
            row(
                b"raw-\xff/file",
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 9,
                },
            ),
        ],
    )
    .unwrap();

    assert_eq!(index.policy_version(), 1);
    assert_eq!(index.source_content_commitment(), &[3; 32]);
    assert_eq!(
        index
            .rows()
            .map(|row| row.relative_path().to_vec())
            .collect::<Vec<_>>(),
        vec![
            b"".to_vec(),
            b"a:b".to_vec(),
            b"raw-\xff".to_vec(),
            b"raw-\xff/file".to_vec()
        ]
    );
}

#[test]
pub(crate) fn canonical_metadata_rejects_invalid_and_duplicate_paths() {
    for invalid in [
        b"/absolute".as_slice(),
        b"a//b".as_slice(),
        b"a/./b".as_slice(),
        b"a/../b".as_slice(),
        b"a\\b".as_slice(),
        b"a\0b".as_slice(),
    ] {
        assert!(matches!(
            CanonicalFilesystemMetadataIndex::version_1(
                [0; 32],
                [
                    row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                    row(invalid, CanonicalFilesystemMetadataRowKind::Directory),
                ],
            ),
            Err(CanonicalFilesystemMetadataIndexError::InvalidRelativePath(path))
                if path == invalid
        ));
    }
    assert!(matches!(
        CanonicalFilesystemMetadataIndex::version_1(
            [0; 32],
            [
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
            ],
        ),
        Err(CanonicalFilesystemMetadataIndexError::DuplicateRelativePath(path))
            if path.is_empty()
    ));
}

#[test]
pub(crate) fn canonical_metadata_requires_one_directory_root_and_directory_parent_closure() {
    assert!(matches!(
        CanonicalFilesystemMetadataIndex::version_1([0; 32], []),
        Err(CanonicalFilesystemMetadataIndexError::MissingRootDirectory)
    ));
    assert!(matches!(
        CanonicalFilesystemMetadataIndex::version_1(
            [0; 32],
            [row(
                b"",
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 0,
                },
            )],
        ),
        Err(CanonicalFilesystemMetadataIndexError::RootIsNotDirectory)
    ));
    assert!(matches!(
        CanonicalFilesystemMetadataIndex::version_1(
            [0; 32],
            [
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                row(
                    b"missing/leaf",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                ),
            ],
        ),
        Err(CanonicalFilesystemMetadataIndexError::MissingParentDirectory(path))
            if path == b"missing/leaf"
    ));
    assert!(matches!(
        CanonicalFilesystemMetadataIndex::version_1(
            [0; 32],
            [
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                row(
                    b"file",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 0,
                    },
                ),
                row(b"file/child", CanonicalFilesystemMetadataRowKind::Directory),
            ],
        ),
        Err(CanonicalFilesystemMetadataIndexError::ParentIsNotDirectory(path))
            if path == b"file/child"
    ));
}

#[test]
pub(crate) fn canonical_metadata_rejects_lengths_outside_the_stat_domain() {
    assert!(matches!(
        CanonicalFilesystemMetadataIndex::version_1(
            [0; 32],
            [
                row(b"", CanonicalFilesystemMetadataRowKind::Directory),
                row(
                    b"huge",
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: i64::MAX as u64 + 1,
                    },
                ),
            ],
        ),
        Err(CanonicalFilesystemMetadataIndexError::LogicalByteLengthExceedsI64(path))
            if path == b"huge"
    ));
}

#[test]
pub(crate) fn canonical_metadata_rejects_more_rows_than_the_resolver_can_issue() {
    let rows = std::iter::once(row(b"", CanonicalFilesystemMetadataRowKind::Directory)).chain(
        (0..CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT).map(|index| {
            CanonicalFilesystemMetadataRow::new(
                format!("entry-{index}").into_bytes(),
                CanonicalFilesystemMetadataRowKind::File {
                    executable: false,
                    logical_byte_length: 0,
                },
            )
        }),
    );

    assert_eq!(
        CanonicalFilesystemMetadataIndex::version_1([0; 32], rows),
        Err(CanonicalFilesystemMetadataIndexError::RowLimitExceeded {
            limit: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT,
            attempted: CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT + 1,
        })
    );
}
