//! Captured source tests.

use super::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind, CapturedBuildSourceInput,
    CapturedSourceEntry, CapturedSourceEntryKind,
};
use checked_interpreter::{CanonicalFilesystemMetadataIndexError, CanonicalFilesystemMetadataRow};
use std::path::{Path, PathBuf};
use std::sync::Arc;
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

/// The retained entry rows of an input as a `from_capture_rows` operand.
fn entry_rows(input: &CapturedBuildSourceInput) -> Vec<(Vec<u8>, CapturedSourceEntry)> {
    input.entries.clone().into_iter().collect()
}

/// The canonical index rows of an input as `version_1` operands.
fn index_rows(input: &CapturedBuildSourceInput) -> Vec<CanonicalFilesystemMetadataRow> {
    input.canonical_source_metadata().rows().collect()
}

#[test]
fn captured_build_source_input_rejects_every_one_field_substitution() {
    let baseline = input_with_template();
    let baseline_index = baseline.canonical_source_metadata().clone();
    assert_eq!(baseline.entry_count(), 6);
    assert_eq!(baseline.file_bytes(), 14);

    // ── Every `CapturedSourceEntry` field whose substitution stays inside
    //    the joined index: admission accepts the canonical shape, the
    //    retained evidence digest diverges, and the substituted record no
    //    longer equals the baseline. ──

    // File bytes, same length: the extent join still holds, so admission
    // accepts; the honestly recomputed content digest diverges the record.
    let mut rows = entry_rows(&baseline);
    for (path, entry) in rows.iter_mut() {
        if path == b"templates/banner.tmpl" {
            *entry = CapturedSourceEntry::file(b"HELLO WORLE".to_vec(), false);
        }
    }
    let substituted = CapturedBuildSourceInput::from_capture_rows(baseline_index.clone(), rows)
        .expect("a same-length byte substitution keeps the joined extent");
    assert_ne!(substituted, baseline);
    assert_eq!(substituted.file_bytes(), baseline.file_bytes());
    assert_ne!(
        substituted
            .entries
            .get(b"templates/banner.tmpl".as_slice())
            .and_then(CapturedSourceEntry::content_digest),
        baseline
            .entries
            .get(b"templates/banner.tmpl".as_slice())
            .and_then(CapturedSourceEntry::content_digest),
        "the retained content digest must diverge with the captured bytes"
    );
    // Materialization preserves the substitution faithfully: the
    // snapshot bytes differ from the baseline even though the index join
    // is unchanged. Only the retained digest carries the difference.
    let fixture = Fixture::new("bytes-substituted");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");
    // `input_with_template` carries an executable file this host may not
    // represent; apply the substitution over the portable inventory.
    let mut portable_rows = entry_rows(&ordinary_input());
    for (path, entry) in portable_rows.iter_mut() {
        if path == b"templates/banner.tmpl" {
            *entry = CapturedSourceEntry::file(b"HELLO WORLE".to_vec(), false);
        }
    }
    let portable = CapturedBuildSourceInput::from_capture_rows(
        ordinary_input().canonical_source_metadata().clone(),
        portable_rows,
    )
    .expect("portable byte substitution");
    portable
        .materialize_into(&backing)
        .expect("an honest byte substitution materializes");
    assert_eq!(
        std::fs::read(backing.join("templates/banner.tmpl")).expect("read materialized template"),
        b"HELLO WORLE"
    );

    // A forged content digest that disagrees with the retained bytes is
    // representable at admission (the digest is re-inspected, not joined),
    // but the materialization re-inspection rejects it after writing.
    let mut forged_rows = entry_rows(&ordinary_input());
    for (path, entry) in forged_rows.iter_mut() {
        if path == b"templates/banner.tmpl" {
            *entry = CapturedSourceEntry::File {
                bytes: Arc::from(b"HELLO WORLD".as_slice()),
                digest: [0xEE; 32],
                executable: false,
            };
        }
    }
    let forged = CapturedBuildSourceInput::from_capture_rows(
        ordinary_input().canonical_source_metadata().clone(),
        forged_rows,
    )
    .expect("a forged digest keeps the joined extent");
    assert_ne!(forged, ordinary_input());
    let fixture = Fixture::new("forged-digest");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");
    let error = forged
        .materialize_into(&backing)
        .expect_err("the re-inspected digest must reject a forged one");
    assert!(
        error.message().contains("content drifted"),
        "unexpected rejection: {}",
        error.message()
    );

    // Symlink target, same length: the extent join still holds; the
    // retained spelling digest diverges. Captured links stay inert, so
    // materialization never re-inspects them — the divergence is
    // identity-bound through the record, not execution-derived.
    let mut rows = entry_rows(&baseline);
    for (path, entry) in rows.iter_mut() {
        if path == b"link" {
            *entry = CapturedSourceEntry::symlink(b"templater".to_vec());
        }
    }
    let substituted = CapturedBuildSourceInput::from_capture_rows(baseline_index.clone(), rows)
        .expect("a same-length target substitution keeps the joined extent");
    assert_ne!(substituted, baseline);
    assert_ne!(
        substituted
            .entries
            .get(b"link".as_slice())
            .and_then(CapturedSourceEntry::content_digest),
        baseline
            .entries
            .get(b"link".as_slice())
            .and_then(CapturedSourceEntry::content_digest),
        "the retained link spelling digest must diverge"
    );

    // A forged link digest is likewise identity-bound only: admission
    // accepts it and materialization skips inert links, so the record
    // carries the substitution through its own equality.
    let mut rows = entry_rows(&baseline);
    for (path, entry) in rows.iter_mut() {
        if path == b"link" {
            *entry = CapturedSourceEntry::Symlink {
                target: Arc::from(b"templates".as_slice()),
                digest: [0xEE; 32],
            };
        }
    }
    let substituted = CapturedBuildSourceInput::from_capture_rows(baseline_index.clone(), rows)
        .expect("a forged inert digest keeps the joined extent");
    assert_ne!(substituted, baseline);

    // ── Admission-side substitutions that cannot keep the joined index:
    //    each rejects inside `from_capture_rows`. The `entries` roster is a
    //    BTreeMap, so a reordered roster is not representable; every other
    //    axis is. ──
    let cases: Vec<(
        &'static str,
        Vec<(Vec<u8>, CapturedSourceEntry)>,
        &'static str,
    )> = vec![
        (
            "entry path renamed",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"templates/banner.tmpl" {
                        (b"templates/renamed.tmpl".to_vec(), entry)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "absent from its canonical source metadata index",
        ),
        (
            "entry path escapes the root",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (b"../escape".to_vec(), entry)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "noncanonical relative path",
        ),
        (
            "entry path is empty",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (Vec::new(), entry)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "noncanonical relative path",
        ),
        (
            "entry path carries a NUL",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (b"li\0nk".to_vec(), entry)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "noncanonical relative path",
        ),
        (
            "entry path carries a backslash",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (b"tem\\plates".to_vec(), entry)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "noncanonical relative path",
        ),
        (
            "file becomes a directory",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"templates/banner.tmpl" {
                        (path, CapturedSourceEntry::Directory)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "directory becomes a file",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"templates" {
                        (
                            path,
                            CapturedSourceEntry::file(b"NOT A DIRECTORY".to_vec(), false),
                        )
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "file becomes a symlink",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"templates/banner.tmpl" {
                        (path, CapturedSourceEntry::symlink(b"eleven byte".to_vec()))
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "symlink becomes a file",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (
                            path,
                            CapturedSourceEntry::file(b"nine byte".to_vec(), false),
                        )
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "symlink becomes a directory",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (path, CapturedSourceEntry::Directory)
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "file byte length substituted",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"templates/banner.tmpl" {
                        (path, CapturedSourceEntry::file(b"SHORT".to_vec(), false))
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "executable class substituted",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"tools/run.sh" {
                        (path, CapturedSourceEntry::file(b"RUN".to_vec(), false))
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "symlink target length substituted",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"link" {
                        (
                            path,
                            CapturedSourceEntry::symlink(b"too-long-target".to_vec()),
                        )
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
        (
            "file row dropped",
            entry_rows(&baseline)
                .into_iter()
                .filter(|(path, _)| path != b"templates/banner.tmpl")
                .collect(),
            "omits canonical source file",
        ),
        (
            "directory row dropped",
            entry_rows(&baseline)
                .into_iter()
                .filter(|(path, _)| path != b"tools")
                .collect(),
            "omits canonical source directory",
        ),
        (
            "symlink row dropped",
            entry_rows(&baseline)
                .into_iter()
                .filter(|(path, _)| path != b"link")
                .collect(),
            "omits canonical source symlink",
        ),
        (
            "entry path duplicated",
            {
                let mut rows = entry_rows(&baseline);
                let duplicate = rows
                    .iter()
                    .find(|(path, _)| path == b"tools")
                    .expect("tools row")
                    .clone();
                rows.push(duplicate);
                rows
            },
            "duplicates relative path",
        ),
        (
            "entry outside the index",
            {
                let mut rows = entry_rows(&baseline);
                rows.push((
                    b"extra.txt".to_vec(),
                    CapturedSourceEntry::file(b"EXTRA".to_vec(), false),
                ));
                rows
            },
            "absent from its canonical source metadata index",
        ),
        (
            "file rows swapped between paths",
            entry_rows(&baseline)
                .into_iter()
                .map(|(path, entry)| {
                    if path == b"templates/banner.tmpl" {
                        (path, CapturedSourceEntry::file(b"RUN".to_vec(), true))
                    } else if path == b"tools/run.sh" {
                        (
                            path,
                            CapturedSourceEntry::file(b"HELLO WORLD".to_vec(), false),
                        )
                    } else {
                        (path, entry)
                    }
                })
                .collect(),
            "disagrees with its canonical source metadata kind or extent",
        ),
    ];
    for (name, rows, fragment) in cases {
        let error = CapturedBuildSourceInput::from_capture_rows(baseline_index.clone(), rows)
            .expect_err("the substitution must reject at admission");
        assert!(
            error
                .iter()
                .any(|diagnostic| diagnostic.message.contains(fragment)),
            "{name}: expected `{fragment}` in: {error:?}"
        );
    }

    // ── The `canonical_source_metadata` field: the joined authority. ──
    let baseline_index_rows = index_rows(&baseline);
    let index_substitutions: Vec<(
        &'static str,
        Box<dyn Fn(&mut Vec<CanonicalFilesystemMetadataRow>)>,
        Option<&'static str>,
    )> = vec![
        (
            "index row path renamed",
            Box::new(|rows| {
                for row in rows.iter_mut() {
                    if row.relative_path() == b"templates/banner.tmpl" {
                        *row = CanonicalFilesystemMetadataRow::new(
                            b"templates/renamed.tmpl".to_vec(),
                            row.kind(),
                        );
                    }
                }
            }),
            Some("absent from its canonical source metadata index"),
        ),
        (
            "index row executable class",
            Box::new(|rows| {
                for row in rows.iter_mut() {
                    if row.relative_path() == b"templates/banner.tmpl" {
                        *row = CanonicalFilesystemMetadataRow::new(
                            row.relative_path().to_vec(),
                            CanonicalFilesystemMetadataRowKind::File {
                                executable: true,
                                logical_byte_length: 11,
                            },
                        );
                    }
                }
            }),
            Some("disagrees with its canonical source metadata kind or extent"),
        ),
        (
            "index row file extent",
            Box::new(|rows| {
                for row in rows.iter_mut() {
                    if row.relative_path() == b"templates/banner.tmpl" {
                        *row = CanonicalFilesystemMetadataRow::new(
                            row.relative_path().to_vec(),
                            CanonicalFilesystemMetadataRowKind::File {
                                executable: false,
                                logical_byte_length: 12,
                            },
                        );
                    }
                }
            }),
            Some("disagrees with its canonical source metadata kind or extent"),
        ),
        (
            "index row symlink extent",
            Box::new(|rows| {
                for row in rows.iter_mut() {
                    if row.relative_path() == b"link" {
                        *row = CanonicalFilesystemMetadataRow::new(
                            row.relative_path().to_vec(),
                            CanonicalFilesystemMetadataRowKind::Symlink {
                                target_spelling_logical_byte_length: 10,
                            },
                        );
                    }
                }
            }),
            Some("disagrees with its canonical source metadata kind or extent"),
        ),
        (
            "index row kind swapped",
            Box::new(|rows| {
                for row in rows.iter_mut() {
                    if row.relative_path() == b"templates/banner.tmpl" {
                        *row = CanonicalFilesystemMetadataRow::new(
                            row.relative_path().to_vec(),
                            CanonicalFilesystemMetadataRowKind::Directory,
                        );
                    }
                }
            }),
            Some("disagrees with its canonical source metadata kind or extent"),
        ),
        (
            // The retained entry loses its joined row, so the entry-side
            // absence check fires before the index-coverage check.
            "index row dropped",
            Box::new(|rows| {
                rows.retain(|row| row.relative_path() != b"tools/run.sh");
            }),
            Some("absent from its canonical source metadata index"),
        ),
        (
            "index row inserted",
            Box::new(|rows| {
                rows.push(CanonicalFilesystemMetadataRow::new(
                    b"extra.txt".to_vec(),
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 1,
                    },
                ));
            }),
            Some("omits canonical source file"),
        ),
        (
            // The source-content commitment is opaque to this join — the
            // record accepts it — but it is the published source identity,
            // so a substituted commitment cannot reproduce the baseline
            // record's canonical metadata.
            "index source_content_commitment",
            Box::new(|_rows| {}),
            None,
        ),
    ];
    for (name, mutate, fragment) in index_substitutions {
        let mut rows = baseline_index_rows.clone();
        mutate(&mut rows);
        let commitment = if name == "index source_content_commitment" {
            [0xEE; 32]
        } else {
            *baseline_index.source_content_commitment()
        };
        let index = CanonicalFilesystemMetadataIndex::version_1(commitment, rows)
            .expect("the substituted index still encodes canonically");
        match fragment {
            Some(fragment) => {
                let error =
                    CapturedBuildSourceInput::from_capture_rows(index, entry_rows(&baseline))
                        .expect_err("the substituted index must reject the retained entries");
                assert!(
                    error
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains(fragment)),
                    "{name}: expected `{fragment}` in: {error:?}"
                );
            }
            None => {
                let substituted =
                    CapturedBuildSourceInput::from_capture_rows(index, entry_rows(&baseline))
                        .expect("an opaque commitment substitution still admits");
                assert_ne!(
                    substituted.canonical_source_metadata(),
                    baseline.canonical_source_metadata(),
                    "{name}: the substituted containing identity must diverge"
                );
                assert_ne!(substituted, baseline, "{name}");
            }
        }
    }

    // The index itself refuses noncanonical encodings; its own matrix lives
    // in checked-interpreter. The version axis is exercised here because it
    // is the one encoding rejection that record's family does not cover.
    assert_eq!(
        CanonicalFilesystemMetadataIndex::new(2, [0x42; 32], baseline_index_rows.clone(),),
        Err(CanonicalFilesystemMetadataIndexError::UnsupportedPolicyVersion(2)),
        "a noncanonical policy version rejects at index encoding"
    );

    // ── The `file_bytes` projection field is a derived cache: a forged
    //    value diverges the record identity but no local execution re-derives
    //    it — it stays identity-bound, not execution-derived. ──
    let mut forged = baseline.clone();
    forged.file_bytes += 1;
    assert_ne!(forged, baseline);
    assert_eq!(forged.file_bytes(), baseline.file_bytes() + 1);

    // ── Record-level mutations in place: substituting the joined `entries`
    //    or `canonical_source_metadata` fields diverges the whole record
    //    even when every join still holds. ──
    let mut substituted = baseline.clone();
    substituted.entries.insert(
        b"link".to_vec(),
        CapturedSourceEntry::symlink(b"templater".to_vec()),
    );
    assert_ne!(substituted, baseline);
    let mut substituted = baseline.clone();
    substituted.canonical_source_metadata =
        CanonicalFilesystemMetadataIndex::version_1([0xEE; 32], baseline_index_rows)
            .expect("substituted index");
    assert_ne!(substituted, baseline);
}
