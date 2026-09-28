//! Captured source tests.

mod captured_source_custody_fields;

use super::{
    CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRowKind, CapturedBuildSourceInput,
    CapturedSourceEntry, CapturedSourceEntryKind,
};
use captured_source_custody_fields::{
    CapturedSourceCheck, CapturedSourceFieldForTest, captured_source_admission_kind,
    captured_source_custody_outcome, captured_source_materialization_kind,
    substitute_captured_source_for_test,
};
use checked_interpreter::{CanonicalFilesystemMetadataIndexError, CanonicalFilesystemMetadataRow};
use optimization_core::{OneFieldSubstitutionMatrix, run_one_field_substitution_matrix};
use std::cell::Cell;
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
    } else if !metadata.file_type().is_symlink() {
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

#[cfg(unix)]
#[test]
fn discard_never_rewrites_modes_through_planted_aliases() {
    use std::os::unix::fs::PermissionsExt;

    let input = ordinary_input();
    let fixture = Fixture::new("discard-alias");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");
    input
        .materialize_into(&backing)
        .expect("materialize captured source input");

    let sentinel = fixture.0.join("sentinel");
    std::fs::write(&sentinel, b"x").expect("write outside sentinel");
    std::fs::set_permissions(&sentinel, std::fs::Permissions::from_mode(0o600))
        .expect("seal sentinel mode");
    std::fs::set_permissions(&backing, std::fs::Permissions::from_mode(0o755))
        .expect("unseal the sealed backing for the planted entry");
    std::os::unix::fs::symlink(&sentinel, backing.join("planted"))
        .expect("plant an alias inside the private tree");
    let linked = fixture.0.join("linked-sentinel");
    std::fs::write(&linked, b"y").expect("write linked sentinel");
    std::fs::set_permissions(&linked, std::fs::Permissions::from_mode(0o600))
        .expect("seal linked sentinel mode");
    std::fs::hard_link(&linked, backing.join("shared"))
        .expect("plant a hard link inside the private tree");

    super::discard_materialized_snapshot(&backing).expect("discard snapshot");

    assert_eq!(
        std::fs::symlink_metadata(&sentinel)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600,
        "discard must never chmod through a symlink alias out of the tree"
    );
    assert_eq!(
        std::fs::symlink_metadata(&linked)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600,
        "discard must never chmod through a hard-link alias out of the tree"
    );
    assert!(
        std::fs::symlink_metadata(&backing).is_err(),
        "the private tree is still removed"
    );

    let victim = fixture.0.join("victim");
    std::fs::create_dir(&victim).expect("create outside victim directory");
    std::fs::write(victim.join("kept"), b"keep").expect("write kept file");
    let victim_mode = std::fs::symlink_metadata(&victim)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    let alias = fixture.0.join("alias-destination");
    std::os::unix::fs::symlink(&victim, &alias).expect("plant alias destination");

    super::discard_materialized_snapshot(&alias).expect("discard alias destination");

    assert_eq!(
        std::fs::symlink_metadata(&victim)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        victim_mode,
        "an aliased destination never rewrites the target directory's mode"
    );
    assert!(
        victim.join("kept").exists(),
        "the aliased directory's contents survive"
    );
    assert!(
        std::fs::symlink_metadata(&alias).is_err(),
        "the alias itself is unlinked rather than followed"
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

/// The family's independent checker: re-admit the record's own index and
/// entries, prove the admission-derived projection matches what the record
/// carries, re-inspect the admitted record by materialization, and compare
/// the surviving record against the retained baseline evidence — the
/// boundary a substitution that binds different content to the published
/// index can never cross.
fn replay_captured_record(
    baseline: &CapturedBuildSourceInput,
    fresh_backing: &dyn Fn() -> PathBuf,
    record: &CapturedBuildSourceInput,
) -> Result<CapturedBuildSourceInput, CapturedSourceCheck> {
    let rebuilt = CapturedBuildSourceInput::from_capture_rows(
        record.canonical_source_metadata().clone(),
        entry_rows(record),
    )
    .map_err(|diagnostics| {
        CapturedSourceCheck::Admission(captured_source_admission_kind(&diagnostics))
    })?;
    if rebuilt != *record {
        return Err(CapturedSourceCheck::ProjectionDiverged);
    }
    let backing = fresh_backing();
    record.materialize_into(&backing).map_err(|error| {
        CapturedSourceCheck::Materialization(captured_source_materialization_kind(&error))
    })?;
    if *record != *baseline {
        return Err(CapturedSourceCheck::DivergentEvidence);
    }
    Ok(record.clone())
}

#[test]
fn captured_build_source_input_rejects_every_one_field_substitution() {
    let baseline = input_with_template();
    assert_eq!(baseline.entry_count(), 6);
    assert_eq!(baseline.file_bytes(), 14);

    // Every declared field: the substitution changes the retained record
    // (the custody view is the record itself), the containing index and the
    // `file_bytes` projection re-derive honestly inside re-admission, and
    // the independent checker names the stage that refuses it. Rejected
    // materializations seal their written entries read-only, so each leg
    // gets a fresh backing under one fixture root whose drop unseals and
    // removes them all.
    let replay_root = Fixture::new("replay-backings");
    let backing_counter = Cell::new(0u64);
    let fresh_backing = || {
        let backing = replay_root
            .0
            .join(format!("replay-{}", backing_counter.get()));
        backing_counter.set(backing_counter.get() + 1);
        std::fs::create_dir(&backing).expect("create fresh replay backing");
        backing
    };
    let check = |record: &CapturedBuildSourceInput| {
        replay_captured_record(&baseline, &fresh_backing, record)
    };

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "captured build source input",
        fields: CapturedSourceFieldForTest::INVENTORY,
        honest: &|| baseline.clone(),
        donor: ordinary_input(),
        custody: &|record: &CapturedBuildSourceInput| record.clone(),
        substitute: &substitute_captured_source_for_test,
        check: &check,
        outcome: &captured_source_custody_outcome,
        joined_replay: None,
    });

    // ── What stays authored beside the matrix is not a one-field
    //    substitution of the record: ──

    // A duplicated relative path is representable only in the input rows
    // domain — the record's `BTreeMap` roster cannot hold it — so this leg
    // exercises `from_capture_rows` directly and names the input admission
    // rejection.
    let mut rows = entry_rows(&baseline);
    let duplicate = rows
        .iter()
        .find(|(path, _)| path == b"tools")
        .expect("tools row")
        .clone();
    rows.push(duplicate);
    let error = CapturedBuildSourceInput::from_capture_rows(
        baseline.canonical_source_metadata().clone(),
        rows,
    )
    .expect_err("a duplicated relative path rejects at admission");
    assert!(
        error
            .iter()
            .any(|diagnostic| diagnostic.message.contains("duplicates relative path")),
        "expected `duplicates relative path` in: {error:?}"
    );

    // The index itself refuses noncanonical encodings; its own matrix lives
    // in checked-interpreter. The version axis is exercised here because it
    // is the one encoding rejection that record's family does not cover.
    let baseline_index_rows = index_rows(&baseline);
    assert_eq!(
        CanonicalFilesystemMetadataIndex::new(2, [0x42; 32], baseline_index_rows.clone(),),
        Err(CanonicalFilesystemMetadataIndexError::UnsupportedPolicyVersion(2)),
        "a noncanonical policy version rejects at index encoding"
    );

    // Materialization preserves an honest substitution faithfully: the
    // snapshot bytes differ from the baseline even though the index join is
    // unchanged. `input_with_template` carries an executable file this host
    // may not represent, so the read-back evidence applies the substitution
    // over the portable inventory.
    let fixture = Fixture::new("bytes-substituted");
    let backing = fixture.0.join("snapshot");
    std::fs::create_dir(&backing).expect("create empty backing");
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
}
